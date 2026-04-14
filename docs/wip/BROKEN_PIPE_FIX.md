# BrokenPipe Error - Quick Fix

## Immediate Workaround

The BrokenPipe "read closed" errors from may_minihttp are normal client disconnections being logged as ERROR.

### Quick Fix - Set Environment Variable

Add this to suppress the noisy ERROR logs from may_minihttp:

```bash
# Suppress may_minihttp connection close errors
export RUST_LOG="info,may_minihttp::http_server=warn"

# Or for more granular control:
export RUST_LOG="info,may_minihttp=warn,brrtrouter=debug"
```

### In Docker/Kubernetes

Add to your deployment:

```yaml
env:
  - name: RUST_LOG
    value: "info,may_minihttp::http_server=warn"
```

### Testing the Fix

1. Set the environment variable:
   ```bash
   export RUST_LOG="info,may_minihttp::http_server=warn"
   ```

2. Run pet_store:
   ```bash
   cd examples/pet_store
   cargo run --release -- --spec doc/openapi.yaml --test-api-key test123
   ```

3. Test with connection closes:
   ```bash
   # This will timeout and close connection
   curl --max-time 0.5 http://localhost:8080/pets
   
   # Or use telnet and disconnect
   telnet localhost 8080
   GET /pets HTTP/1.1
   ^C  # Ctrl+C to disconnect
   ```

4. Check logs - you should NOT see the BrokenPipe ERROR messages

## Permanent Fix Options

### Option 1: Update LogConfig in otel.rs

We can enhance the LogConfig to properly apply target filters:

```rust
// In src/otel.rs, update init_logging_with_config:
let mut env_filter = EnvFilter::try_from_default_env()
    .unwrap_or_else(|_| EnvFilter::new(level.as_str()));

// Add may_minihttp filter
env_filter = env_filter.add_directive(
    "may_minihttp::http_server=warn".parse().unwrap()
);

// Apply custom target filters if provided
if let Some(target_filter) = &config.target_filter {
    for filter in target_filter.split(',') {
        if let Ok(directive) = filter.parse() {
            env_filter = env_filter.add_directive(directive);
        }
    }
}
```

### Option 2: Fork may_minihttp

1. Fork https://github.com/microscaler/may_minihttp
2. Change line 282 in src/http_server.rs:
   ```rust
   // OLD:
   error!("service err = {e:?}");
   
   // NEW:
   match e.kind() {
       io::ErrorKind::BrokenPipe | 
       io::ErrorKind::ConnectionAborted | 
       io::ErrorKind::ConnectionReset => {
           debug!("connection closed: {e:?}");
       }
       _ => error!("service err = {e:?}"),
   }
   ```
3. Update Cargo.toml to use fork

### Option 3: PR to Upstream

Submit a PR to may_minihttp with the proper fix. This is the best long-term solution.

## Why This Matters

1. **Log Noise**: These errors pollute production logs
2. **False Alerts**: Monitoring systems may trigger on ERROR count
3. **Debugging**: Real errors get lost in the noise
4. **Performance**: No actual performance impact, just log spam

## Verification

After applying the fix, you should see:
- Clean logs with only real errors
- Connection closes logged at DEBUG/WARN level
- Monitoring systems showing accurate error rates
