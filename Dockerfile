FROM rust:1.59 as builder
WORKDIR /usr/src/ktn
COPY . .
RUN rm .env && mv .env.build .env
RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update && apt-get install -y sqlite3
COPY --from=builder /usr/local/cargo/bin/ktn /usr/local/bin/ktn
RUN mkdir -p /usr/local/share/ktn/
COPY static /usr/local/share/ktn/static
EXPOSE 8080
EXPOSE 2525
CMD ["ktn"]
