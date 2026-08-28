FROM rust:1-bookworm AS builder

WORKDIR /app

COPY . .

RUN cargo build --release --bin titan-control-plane


FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder \
    /app/target/release/titan-control-plane \
    /usr/local/bin/titan-control-plane

EXPOSE 8080

ENV RUST_LOG=info

CMD ["titan-control-plane"]
