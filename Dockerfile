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

# Build the example crate with musl target
RUN cargo build --release -p pet_store --target x86_64-unknown-linux-musl

# ---- Runtime stage: minimal scratch image with only the binary and assets ----
FROM scratch
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/pet_store /pet_store
COPY --from=builder /build/examples/pet_store/doc /doc
COPY --from=builder /build/examples/pet_store/static_site /static_site

EXPOSE 8080
ENV BRRTR_LOCAL=1
ENTRYPOINT ["/pet_store"]
