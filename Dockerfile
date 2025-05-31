FROM debian:buster-slim
WORKDIR /app
COPY ./target/release/pet_store /app/pet_store
COPY ./examples/pet_store/doc /app/doc
COPY ./examples/pet_store/static_site /app/static_site

EXPOSE 8080
ENV BRRTR_LOCAL=1
CMD ["/app/pet_store"]
