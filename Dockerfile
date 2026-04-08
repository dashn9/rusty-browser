# ── Build ──────────────────────────────────────────────────────────────────────
FROM rust:1.93-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

RUN cargo build --release -p rustmani-server

# ── Runtime ────────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/rustmani /app/rustmani

EXPOSE 8080

ENV RUSTMANI_CONFIG=/app/rustmani.yaml

ENTRYPOINT ["/app/rustmani"]
