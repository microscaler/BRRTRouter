# BRRTRouter GitHub Workflows

## Build Validator Binaries Workflow

The `build-validator-binaries.yml` workflow builds platform-specific binaries for the BRRTRouter Python validator.

### Process

1. **Build Binaries**: Builds validator binaries for all 7 platforms in parallel:
   - **Container Platforms (K8S)**:
     - Linux AMD64 (`linux/amd64`) - x86_64 containers
     - Linux ARM64 (`linux/arm64`) - ARM64 containers (Apple Silicon, ARM64 servers)
     - Linux ARMv7 (`linux/arm/v7`) - ARMv7 containers (Raspberry Pi)
   - **Developer Workstations**:
     - Windows AMD64
     - Windows ARM64
     - macOS AMD64 (Intel)
     - macOS ARM64 (Apple Silicon)

2. **Cross-Compilation**: 
   - Uses QEMU for ARM platform emulation
   - Installs cross-compilation toolchains (gcc-aarch64-linux-gnu, gcc-arm-linux-gnueabihf)
   - Configures Cargo with appropriate linkers
   - Sets environment variables for cross-compilation

3. **Create Archives**: Creates platform-specific tar.gz archives

4. **Upload to Release**: Uploads all archives to GitHub Release

### Container Platform Support

The workflow specifically supports container platforms used in Kubernetes:
- `linux/amd64` - Standard x86_64 containers
- `linux/arm64` - ARM64 containers (for Apple Silicon Macs running Docker, ARM64 K8S nodes)
- `linux/arm/v7` - ARMv7 containers (for Raspberry Pi, ARMv7 K8S nodes)

These binaries are used by BRRTRouter services running in Docker/K8S, which may run on different node architectures.

### Triggers

- Push to tag matching `v*` (e.g., `v0.1.0`)
- GitHub Release published
- Manual workflow dispatch

### Inputs (workflow_dispatch)

- `version`: Version tag (e.g., v0.1.0). If not provided, extracted from git tag.

### Outputs

For each platform:
- `brrtrouter-validator-<platform>-<version>.tar.gz`

Combined archives:
- `brrtrouter-validator-all-<version>.tar.gz` - All platforms
- `brrtrouter-validator-containers-<version>.tar.gz` - Container platforms only (for K8S)

### Cross-Compilation Details

- **Linux ARM64**: Uses `aarch64-unknown-linux-gnu` target with `aarch64-linux-gnu-gcc` linker
- **Linux ARMv7**: Uses `armv7-unknown-linux-gnueabihf` target with `arm-linux-gnueabihf-gcc` linker
- **QEMU**: Set up for ARM platform emulation during cross-compilation
- **Cargo Config**: Automatically configured with appropriate linker settings

### Dependencies

- Rust toolchain (via `dtolnay/rust-toolchain`)
- Python and maturin for building wheels
- Cross-compilation tools:
  - `gcc-aarch64-linux-gnu` for ARM64
  - `gcc-arm-linux-gnueabihf` for ARMv7
- QEMU (via `docker/setup-qemu-action`) for ARM emulation

### Usage

```bash
# Trigger manually
gh workflow run build-validator-binaries.yml -f version=v0.1.0

# Or create a git tag
git tag v0.1.0
git push origin v0.1.0
```

### Integration with BRRTRouter Services

BRRTRouter services running in Kubernetes will:
1. Detect the container platform at runtime
2. Load the appropriate validator binary from the container-only archive
3. Use it for OpenAPI validation before generating handlers

This ensures optimal performance on each K8S node architecture.
