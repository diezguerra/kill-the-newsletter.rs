FROM rust:1.59-slim-buster as builder
WORKDIR /usr/src/ktn
COPY . .
RUN apt-get update && apt-get install -y pkg-config sqlite3 libssl-dev
RUN rm .env && mv .env.build .env
RUN cargo install --features tracing_json --path .

FROM debian:buster-slim
RUN apt-get update && apt-get install -y sqlite3 libssl-dev
COPY --from=builder /usr/local/cargo/bin/ktn /usr/local/bin/ktn
RUN mkdir -p /usr/local/share/ktn/
COPY static /usr/local/share/ktn/static
EXPOSE 8080
EXPOSE 2525
CMD ["ktn"]
