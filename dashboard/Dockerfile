# ---- Build stage ----
# The pin must be >= the highest `rust-version` in the dependency tree, not just
# whatever built when this file was written. It was `1.86`, and the build failed
# outright — askama 0.15 and time 0.3 both require 1.88 — but nothing ran
# `docker build` until CI did, so the image had simply stopped being buildable.
# See `rust-version` in the workspace Cargo.toml, which is the declaration this
# tracks.
FROM rust:1.90-bookworm AS builder
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# sqlx verifies every query!/query_as! macro against a real database at compile
# time, so the build needs one that already has the tables. Create it from the
# same schema.sql the server applies at startup.
#
# This stage used to create an *empty* database, and the build failed with
# "no such table: projects": the tables only appeared once the server had been
# run, which cannot happen before the binary exists.
RUN apt-get update && apt-get install -y --no-install-recommends sqlite3 \
    && rm -rf /var/lib/apt/lists/*
RUN sqlite3 omnivore-build.db < crates/omnivore-core/schema.sql
ENV DATABASE_URL="sqlite:omnivore-build.db?mode=rwc"

# Build release binary (strip + LTO enabled in Cargo.toml)
RUN cargo build --release --locked

# ---- Runtime stage ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Run as a non-root user; /data is the only path the server writes to.
RUN useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin omnivore

COPY --from=builder /app/target/release/omnivore-dashboard /usr/local/bin/
COPY --from=builder /app/crates/omnivore-server/static /opt/omnivore/static

RUN mkdir -p /data && chown omnivore:omnivore /data
VOLUME ["/data"]
WORKDIR /data
USER omnivore
EXPOSE 3000

ENV DATABASE_URL="sqlite:/data/omnivore.db?mode=rwc"
ENV BIND_ADDR="0.0.0.0:3000"
ENV RUST_LOG="info"
ENV OMNIVORE_STATIC_DIR="/opt/omnivore/static"

CMD ["omnivore-dashboard"]
