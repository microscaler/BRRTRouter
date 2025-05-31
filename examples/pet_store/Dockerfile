FROM rust:1.76 as builder
WORKDIR /usr/src/app

# cache deps
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY templates ./templates
COPY examples/openapi.yaml examples/openapi.yaml

# generate the example project from the latest OpenAPI spec
RUN cargo run --bin brrtrouter-gen -- generate --spec examples/openapi.yaml --force

# build the freshly generated pet_store example
RUN cargo build -p pet_store --release

FROM debian:buster-slim
WORKDIR /app
COPY --from=builder /usr/src/app/target/release/pet_store /app/pet_store
COPY --from=builder /usr/src/app/examples/pet_store/doc /app/doc
COPY --from=builder /usr/src/app/examples/pet_store/static_site /app/static_site
EXPOSE 8080
ENV BRRTR_LOCAL=1
CMD ["/app/pet_store"]
