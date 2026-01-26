# Container Platform Support

The BRRTRouter validator binaries are built for container platforms used in Kubernetes deployments.

## Supported Container Platforms

| Platform | Target Triple | Container Platform | Use Case |
|----------|--------------|-------------------|----------|
| Linux AMD64 | `x86_64-unknown-linux-gnu` | `linux/amd64` | Standard x86_64 containers |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `linux/arm64` | Apple Silicon, ARM64 servers |
| Linux ARMv7 | `armv7-unknown-linux-gnueabihf` | `linux/arm/v7` | Raspberry Pi, ARMv7 devices |

## Build Process

The GitHub Actions workflow builds binaries for all container platforms using:

1. **Native builds** for `linux/amd64` (runs on `ubuntu-latest`)
2. **Cross-compilation** for `linux/arm64` and `linux/arm/v7`:
   - Uses QEMU for emulation
   - Installs cross-compilation toolchains
   - Sets appropriate linker flags

## Usage in Kubernetes

When deploying BRRTRouter services in Kubernetes, the validator binary will be automatically selected based on the node architecture:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: brrtrouter-service
spec:
  template:
    spec:
      containers:
      - name: brrtrouter
        image: brrtrouter:latest
        # The validator binary will match the node architecture
```

## Downloading Binaries

### For Container Deployments

Download the container-specific archive:

```bash
# All container platforms
curl -L https://github.com/microscaler/BRRTRouter/releases/download/v0.1.0/brrtrouter-validator-containers-v0.1.0.tar.gz | tar -xz

# Specific platform
curl -L https://github.com/microscaler/BRRTRouter/releases/download/v0.1.0/brrtrouter-validator-linux-amd64-v0.1.0.tar.gz | tar -xz
```

### Platform Detection

The binary selection should match the container runtime platform:

```bash
# In Dockerfile or init script
ARCH=$(uname -m)
case "$ARCH" in
  x86_64) PLATFORM="linux-amd64" ;;
  aarch64) PLATFORM="linux-arm64" ;;
  armv7l) PLATFORM="linux-armv7" ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Copy appropriate binary
cp vendor/brrtrouter-validator/$PLATFORM/brrtrouter_validator.so /usr/local/lib/
```

## Integration with BRRTRouter Services

BRRTRouter services running in Kubernetes will:

1. Detect the container platform at runtime
2. Load the appropriate validator binary
3. Use it for OpenAPI validation before generating handlers

This ensures optimal performance on each architecture.
