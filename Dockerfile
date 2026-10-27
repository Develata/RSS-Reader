# syntax=docker/dockerfile:1.7

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --home-dir /app rssr \
    && mkdir -p /app/auth /data \
    && chown -R rssr:rssr /app /data \
    && chmod 700 /data

ENV RSS_READER_WEB_BIND=0.0.0.0:8080
ENV RSS_READER_WEB_STATIC_DIR=/app/public
# library.sqlite3, auth.sqlite3 and outbound.json share the owner's data directory.
ENV RSSR_DATA_DIR=/data
USER rssr
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD wget -q -O /dev/null http://127.0.0.1:8080/healthz || exit 1
CMD ["rssr-web"]

# Release/CI images consume the tested Linux distribution from the same workflow run.
FROM runtime AS packaged
COPY package/rssr-web /usr/local/bin/rssr-web
COPY package/rssr-cli /usr/local/bin/rssr-cli
COPY package/public /app/public
COPY package/build.json /app/build.json

FROM rust:1.99-bookworm AS builder

RUN rustup target add wasm32-unknown-unknown
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo install dioxus-cli --version 0.7.9 --locked

WORKDIR /app

COPY . .

ARG RSSR_BUILD_ID
ENV RSSR_BUILD_ID=${RSSR_BUILD_ID}
RUN test -n "$RSSR_BUILD_ID" && test "$RSSR_BUILD_ID" != dev-unknown

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    dx bundle --locked --platform web --package rssr-app --release --debug-symbols false --out-dir /app/web-dist

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --locked -p rssr-web -p rssr-cli --release \
    && cp /app/target/release/rssr-web /app/rssr-web \
    && cp /app/target/release/rssr-cli /app/rssr-cli

# The default target supports local source builds with an explicit identifier.
FROM runtime AS source-built
COPY --from=builder /app/rssr-web /usr/local/bin/rssr-web
COPY --from=builder /app/rssr-cli /usr/local/bin/rssr-cli
COPY --from=builder /app/web-dist/public /app/public
