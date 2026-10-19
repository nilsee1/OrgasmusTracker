# ==============================================================================
# Production Dockerfile for Telegram Orgasmus-Tracker Bot (Rust)
# Multi-stage build, minimal Debian slim runtime, runs as non-root user
# ==============================================================================

# Build stage
FROM rust:1-slim AS builder

WORKDIR /usr/src/app

# Cache dependency compilation
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs
RUN cargo build --release && rm -rf src

# Copy real source code and build production binary
COPY src ./src
RUN touch src/main.rs src/lib.rs && cargo build --release

# Runtime stage
FROM debian:12-slim

ENV TZ=Europe/Berlin

# Install security certificates and timezone data
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user and group (UID/GID 10001)
RUN groupadd -g 10001 appgroup && \
    useradd -u 10001 -g appgroup -s /bin/bash -m appuser

# Set working directory
WORKDIR /app

# Create persistent data directory and set permissions
RUN mkdir -p /app/data && \
    chown -R appuser:appgroup /app

# Copy compiled binary from builder
COPY --from=builder --chown=appuser:appgroup /usr/src/app/target/release/orgasmus-tracker /app/orgasmus-tracker

# Switch to non-root user
USER appuser

# Expose volume mount point for SQLite database
VOLUME ["/app/data"]

# Run the Telegram bot
CMD ["/app/orgasmus-tracker"]
