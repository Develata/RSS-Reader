# syntax=docker/dockerfile:1.7

FROM rust:1.88-bookworm AS builder

RUN rustup target add wasm32-unknown-unknown
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo install dioxus-cli --version 0.7.3 --locked

WORKDIR /app

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    dx bundle --platform web --package rssr-app --release --debug-symbols false --out-dir /app/web-dist

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build -p rssr-web --release \
    && cp /app/target/release/rssr-web /app/rssr-web

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --home-dir /app rssr

COPY --from=builder /app/rssr-web /usr/local/bin/rssr-web
COPY --from=builder /app/web-dist/public /app/public

ENV RSS_READER_WEB_BIND=0.0.0.0:8080
ENV RSS_READER_WEB_STATIC_DIR=/app/public

USER rssr

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD wget -q -O /dev/null http://127.0.0.1:8080/healthz || exit 1

CMD ["rssr-web"]
