FROM rust:1.87-bookworm AS builder

RUN rustup target add wasm32-unknown-unknown
RUN cargo install dioxus-cli --version 0.7.3

WORKDIR /app

COPY . .

RUN dx bundle --platform web --package rssr-app --release --debug-symbols false --out-dir /app/target/web-dist

FROM nginx:1.27-alpine

COPY docker/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/target/web-dist/public /usr/share/nginx/html

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
