# ==============================================================================
# Production Dockerfile for Telegram Orgasmus-Tracker Bot
# Uses Python 3.12 slim, runs as non-root user, and mounts persistent SQLite data
# ==============================================================================

FROM python:3.12-slim

# Set Python environment flags
ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PYTHONPATH=/app \
    TZ=Europe/Berlin

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

# Install dependencies first for Docker layer caching
COPY requirements.txt /app/
RUN pip install --no-cache-dir -r requirements.txt

# Create persistent data directory and set permissions
RUN mkdir -p /app/data && \
    chown -R appuser:appgroup /app

# Copy application source code
COPY src/ /app/src/

# Ensure all application files belong to non-root appuser
RUN chown -R appuser:appgroup /app

# Switch to non-root user
USER appuser

# Expose volume mount point for SQLite database
VOLUME ["/app/data"]

# Run the Telegram bot
CMD ["python", "-m", "src.bot"]

