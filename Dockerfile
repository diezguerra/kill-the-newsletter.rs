FROM node:lts-bookworm-slim AS tailwind
WORKDIR /usr/src/ktn
COPY package.json ./
RUN npm install
COPY tailwind.config.js ./
COPY templates ./templates
RUN npx tailwindcss -i ./templates/input.css -o ./static/main.css

FROM rust:latest AS builder
WORKDIR /usr/src/ktn
COPY . .
RUN apt-get update && apt-get install -y pkg-config && rm -rf /var/lib/apt/lists/*
RUN rm .env && mv .env.build .env
RUN cargo install --features tracing_json --path .

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/ktn /usr/local/bin/ktn
RUN mkdir -p /usr/local/share/ktn/
COPY static /usr/local/share/ktn/static
COPY --from=tailwind /usr/src/ktn/static/main.css /usr/local/share/ktn/static/

EXPOSE 8080
EXPOSE 2525
CMD ["ktn"]
