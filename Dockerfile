# Multi-stage Dockerfile for master_orchestrator and agents.

# ---------------------------
# Builder stage
# ---------------------------
FROM rust:1.82 as builder

WORKDIR /app

# Cache dependencies first
COPY Cargo.toml Cargo.lock ./
COPY core ./core
COPY agents ./agents
COPY tools ./tools
COPY data ./data
COPY docs ./docs
COPY frontend ./frontend

# Build release binaries for orchestrator and agents
RUN cargo build --release -p master_orchestrator -p git_agent -p obsidian_agent -p llm_router_agent

# ---------------------------
# Runtime stage
# ---------------------------
FROM debian:bookworm-slim as runtime

# Install minimal runtime dependencies (including CA certificates for HTTPS calls)
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd -m orchestrator

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /app/target/release/master_orchestrator /usr/local/bin/master_orchestrator
COPY --from=builder /app/target/release/git_agent /usr/local/bin/git_agent
COPY --from=builder /app/target/release/obsidian_agent /usr/local/bin/obsidian_agent
COPY --from=builder /app/target/release/llm_router_agent /usr/local/bin/llm_router_agent

# Copy configuration and frontend assets
COPY --from=builder /app/data ./data
COPY --from=builder /app/frontend ./frontend

# Default environment: production
ENV APP_ENV=prod

# Orchestrator HTTP port
EXPOSE 8181

USER orchestrator

# Entrypoint: run the master_orchestrator binary.
CMD ["master_orchestrator"]