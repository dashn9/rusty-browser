# ── Build ──────────────────────────────────────────────────────────────────────
FROM rust:1.93-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

RUN cargo build --release -p rusty-server

# ── Runtime ────────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/rusty /app/rusty

EXPOSE 1011

# GRPC
EXPOSE 50050

ENTRYPOINT ["/app/rusty"]
