# ---- Builder stage: produce a static musl binary ----
FROM --platform=linux/amd64 rust:1.84-alpine AS builder
RUN apk add --no-cache musl-dev openssl-dev pkgconfig build-base
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /build
ENV CC_x86_64_unknown_linux_musl=gcc \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=gcc \
    RUSTFLAGS="-C target-feature=+crt-static"

# Copy full workspace for simplicity; Docker layer cache will help
COPY . .

# Build the example crate with musl target unless an external binary is provided at build time
ARG PETSTORE_BIN=
RUN if [ -z "$PETSTORE_BIN" ]; then \
      cargo build --release -p pet_store --target x86_64-unknown-linux-musl && \
      chmod +x /build/target/x86_64-unknown-linux-musl/release/pet_store ; \
    else \
      echo "Using provided pet_store binary: $PETSTORE_BIN" && \
      chmod +x "$PETSTORE_BIN" ; \
    fi

# ---- Runtime stage: minimal scratch image with only the binary and assets ----
FROM scratch
# If a binary was provided, copy it; otherwise copy the builder output
ARG PETSTORE_BIN=
COPY --from=builder ${PETSTORE_BIN:-/build/target/x86_64-unknown-linux-musl/release/pet_store} /pet_store
COPY --from=builder /build/examples/pet_store/doc /doc
COPY --from=builder /build/examples/pet_store/static_site /static_site
COPY --from=builder /build/examples/pet_store/config /config

EXPOSE 8080
ENV RUST_BACKTRACE=1
ENV RUST_LOG=debug
ENTRYPOINT ["/pet_store", "--spec", "/doc/openapi.yaml", "--doc-dir", "/doc", "--static-dir", "/static_site", "--config", "/config/config.yaml"]
