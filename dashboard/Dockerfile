# ---- Build stage ----
FROM rust:1.86-bookworm AS builder
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Create a stub SQLite DB for sqlx compile-time checks
RUN apt-get update && apt-get install -y sqlite3 && rm -rf /var/lib/apt/lists/*
RUN sqlite3 omnivore-build.db "SELECT 1;"
ENV DATABASE_URL="sqlite:omnivore-build.db?mode=rwc"

# Build release binary (strip + LTO enabled in Cargo.toml)
RUN cargo build --release

# ---- Runtime stage ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/omnivore-dashboard /usr/local/bin/
COPY --from=builder /app/crates/omnivore-server/static /opt/omnivore/static

WORKDIR /data
EXPOSE 3000

ENV DATABASE_URL="sqlite:/data/omnivore.db?mode=rwc"
ENV BIND_ADDR="0.0.0.0:3000"
ENV RUST_LOG="info"
ENV OMNIVORE_STATIC_DIR="/opt/omnivore/static"

CMD ["omnivore-dashboard"]
