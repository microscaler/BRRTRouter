FROM rust:1.76 as builder
WORKDIR /usr/src/app

# cache deps
COPY Cargo.toml Cargo.lock ./
COPY examples/pet_store/Cargo.toml examples/pet_store/Cargo.toml
COPY src ./src
COPY examples/pet_store ./examples/pet_store
COPY templates ./templates
RUN cargo build -p pet_store --release

FROM debian:buster-slim
WORKDIR /app
COPY --from=builder /usr/src/app/target/release/pet_store /app/pet_store
COPY examples/pet_store/doc /app/doc
EXPOSE 8080
ENV BRRTR_LOCAL=1
CMD ["/app/pet_store"]
