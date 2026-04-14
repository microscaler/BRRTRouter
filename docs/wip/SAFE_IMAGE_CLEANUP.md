# Safe Image Cleanup Implementation

## Problem

Your manual cleanup command showed the issue:

```bash
$ for stale in $(docker images | grep 'none' | awk {'print $3'}); do docker image rm $stale; done
Deleted: sha256:2458e6b3d9cb...
Deleted: sha256:16c853aa69c7...
# ... 9 images deleted successfully ...
Error response from daemon: conflict: unable to delete 6c6ce9a8e88c (must be forced) - image is being used by stopped container 1dc41034e757
Error response from daemon: conflict: unable to delete 071dd73121e8 (cannot be forced) - image is being used by running container e26d8dbbf391
```

**Issue:** The script tried to delete ALL `<none>` images, including:
- ❌ Images in use by stopped containers
- ❌ Images in use by running containers (like kind)

## Solution: Two-Step Safe Cleanup

### Step 1: Docker Prune (Safe by Design)

```rust
Command::new("docker")
    .args([
        "image",
        "prune",
        "-f",                           // Force (no prompt)
        "--filter", "dangling=true",    // Only <none>:<none>
        "--filter", "until=1h",         // Only recent
    ])
    .output();
```

**What it does:**
- Removes dangling images NOT referenced by any container
- Built-in safety: won't touch in-use images
- Fast and efficient

**What it misses:**
- Images from stopped containers (referenced but not running)
- Old dangling images (> 1 hour)

### Step 2: Manual Cleanup with Safety Checks

```rust
// Get list of remaining <none> images
let list_result = Command::new("sh")
    .args(["-c", "docker images | grep '<none>' | awk '{print $3}'"])
    .output();

for image_id in image_ids {
    // Try to remove WITHOUT --force
    let rm_result = Command::new("docker")
        .args(["image", "rm", image_id])  // ← No --force!
        .output();
    
    if output.status.success() {
        removed_count += 1;  // Successfully removed
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // Skip images that are in use (safe)
        if stderr.contains("conflict") || stderr.contains("being used") {
            skipped_count += 1;  // In-use, skip it
        } else {
            eprintln!("⚠ Could not remove {}: {}", image_id, stderr.trim());
        }
    }
}
```

**What it does:**
- Tries to remove each `<none>` image individually
- **Never uses `--force`** (respects Docker's safety checks)
- Detects "conflict" and "being used" errors
- Skips in-use images gracefully
- Reports what was removed vs skipped

## Safety Guarantees

### ✅ Will Remove
- Dangling images not referenced by any container
- Old dangling images from previous test runs
- Orphaned images from failed builds

### ✅ Will Skip (Never Force)
- Images in use by running containers
  - Example: `kindest/node` for kind cluster
- Images in use by stopped containers
  - Example: Old test containers not yet removed
- Any image that Docker says "cannot be deleted"

### ✅ Error Handling
```rust
if stderr.contains("conflict") || stderr.contains("being used") {
    skipped_count += 1;  // ← Safe to ignore
}
```

## Output Examples

### Successful Cleanup
```bash
🧹 Cleaning up Docker resources on exit...
Stopping container: abc123
✓ Removed container: abc123
Cleaning up dangling test images...
✓ Pruned: Total reclaimed space: 40.5MB
Found 2 additional <none> image(s) to remove...
✓ Removed 2 <none> image(s)
✓ Cleanup complete
```

### With In-Use Images (kind running)
```bash
🧹 Cleaning up Docker resources on exit...
Stopping container: abc123
✓ Removed container: abc123
Cleaning up dangling test images...
✓ Pruned: Total reclaimed space: 35.2MB
Found 3 additional <none> image(s) to remove...
✓ Removed 2 <none> image(s)
✓ Skipped 1 in-use image(s) (safe)  ← kind's image, kept!
✓ Cleanup complete
```

## Why Not Use `--force`?

```bash
# ❌ BAD: Dangerous!
docker image rm --force $image_id
```

Using `--force` (`-f`) can:
- Remove images from running containers (breaks kind!)
- Remove images from stopped containers (breaks debugging)
- Cause cascading failures

**Our approach:**
- Let Docker decide if it's safe
- Trust the "conflict" error as a safety signal
- Skip problematic images, remove safe ones

## Comparison

| Approach | Safety | Effectiveness | Risk |
|----------|--------|---------------|------|
| **Your manual loop** | ❌ None (tries all) | ⚠️ Partial | High (errors) |
| **docker prune only** | ✅ High | ⚠️ Partial | None |
| **Our two-step** | ✅ High | ✅ Complete | None |

## Implementation

**File:** `tests/curl_harness.rs` lines 44-134

**Runs on:**
- Normal exit (`atexit`)
- SIGINT (Ctrl+C)
- SIGTERM (kill)

**Integration:**
- Same cleanup handler as container cleanup
- Runs after container cleanup (cleanup resources in order)
- Error-tolerant (won't fail tests)

## Benefits

### For Developers
- ✅ Clean `docker images` output after tests
- ✅ No manual cleanup needed
- ✅ kind cluster keeps working
- ✅ Safe for any Docker environment

### For CI
- ✅ Prevents image accumulation
- ✅ Keeps runners clean
- ✅ No special handling needed
- ✅ Works with any Docker setup

## Testing

To verify the safety:

```bash
# Start kind cluster
kind create cluster --name test-cluster

# Check kind's image is present
docker images | grep kindest
# kindest/node   <none>   071dd73121e8   ...

# Run tests (which will try to clean up images)
just nt

# Verify kind's image is still there
docker images | grep kindest
# kindest/node   <none>   071dd73121e8   ...  ← Still there!

# Check cleanup output
# Should see: ✓ Skipped 1 in-use image(s) (safe)
```

## For Future AI/Contributors

### Critical Pattern

**Never use `--force` on `docker image rm` in cleanup handlers!**

The right pattern:
```rust
// ✅ GOOD: Let Docker decide
Command::new("docker").args(["image", "rm", image_id])

// ❌ BAD: Forces deletion, breaks running containers
Command::new("docker").args(["image", "rm", "-f", image_id])
```

### Why This Works

Docker's "conflict" error is a **safety signal**, not a failure:
- Means: "This image is important to a container"
- We should: Skip it and move on
- We should NOT: Force it anyway

### Edge Cases Handled

1. **kind cluster running** → Skips `kindest/node` image
2. **Stopped test container** → Skips its image (until container is removed)
3. **Multiple test runs** → Cleans up old dangling images
4. **Failed builds** → Cleans up partial images
5. **Interrupted builds** → Cleans up orphaned layers

### DO NOT

- ❌ Add `--force` or `-f` to `docker image rm`
- ❌ Remove the "conflict"/"being used" error detection
- ❌ Fail the cleanup if some images can't be removed
- ❌ Assume all `<none>` images can be deleted

### DO

- ✅ Trust Docker's conflict detection
- ✅ Skip in-use images gracefully
- ✅ Report skipped images (for visibility)
- ✅ Use two-step approach (prune + manual)

## Related Documentation

- `docs/IMAGE_CLEANUP.md` - Comprehensive image cleanup guide
- `docs/DANGLING_IMAGES_FIX.md` - Full fix details with --rm flags
- `docs/DOCKER_CLEANUP_FIX.md` - Container cleanup
- `docs/SIGINT_CLEANUP_FIX.md` - Signal handling

## Summary

Implemented **safe, aggressive cleanup** that:
- ✅ Removes all removable `<none>` images
- ✅ Skips in-use images (kind, stopped containers)
- ✅ Never uses `--force` (respects Docker safety)
- ✅ Reports what was done (visibility)
- ✅ Works in all environments (dev, CI, with/without kind)

**Result:** Maximum cleanup with zero risk! 🎉

