# Optimized multi-stage Dockerfile for master_orchestrator and agents
# Focuses on build speed, image size, and production readiness

# ---------------------------
# Dependency cache stage
# ---------------------------
FROM rust:1.82-slim as deps
WORKDIR /app

# Create dummy source files to build dependencies only
RUN mkdir -p core/master_orchestrator/src core/platform/src \
    agents/git_agent/src agents/obsidian_agent/src agents/llm_router_agent/src
RUN touch core/master_orchestrator/src/lib.rs core/platform/src/lib.rs \
    agents/git_agent/src/main.rs agents/obsidian_agent/src/main.rs \
    agents/llm_router_agent/src/main.rs

# Copy manifests for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY core/master_orchestrator/Cargo.toml core/master_orchestrator/
COPY core/platform/Cargo.toml core/platform/
COPY agents/git_agent/Cargo.toml agents/git_agent/
COPY agents/obsidian_agent/Cargo.toml agents/obsidian_agent/
COPY agents/llm_router_agent/Cargo.toml agents/llm_router_agent/

# Build dependencies only
RUN cargo build --release -p master_orchestrator -p git_agent -p obsidian_agent -p llm_router_agent \
    && rm -rf target/release/.fingerprint/*/build-script-build \
    && rm -rf target/release/build/* \
    && rm -rf target/release/deps/*-*.d

# ---------------------------
# Builder stage
# ---------------------------
FROM rust:1.82-slim as builder
WORKDIR /app

# Copy pre-built dependencies
COPY --from=deps /app/target target
COPY --from=deps /usr/local/cargo /usr/local/cargo

# Copy actual source code
COPY Cargo.toml Cargo.lock ./
COPY core ./core
COPY agents ./agents

# Optimize build flags to reduce binary size
ENV RUSTFLAGS="-C link-arg=-s -C opt-level=3 -C codegen-units=1 -C lto=fat"

# Build release binaries with optimizations
RUN cargo build --release --offline -p master_orchestrator -p git_agent -p obsidian_agent -p llm_router_agent

# Strip debug symbols for smaller binaries
RUN strip target/release/master_orchestrator \
    target/release/git_agent \
    target/release/obsidian_agent \
    target/release/llm_router_agent

# ---------------------------
# Runtime stage (Alpine for minimal size)
# ---------------------------
FROM alpine:3.19 as runtime

# Install minimal runtime dependencies
RUN apk add --no-cache ca-certificates tzdata libgcc

# Create non-root user for security
RUN addgroup -S orchestrator && adduser -S -G orchestrator orchestrator

WORKDIR /app

# Copy only the necessary files from builder
COPY --from=builder /app/target/release/master_orchestrator /usr/local/bin/master_orchestrator
COPY --from=builder /app/target/release/git_agent /usr/local/bin/git_agent
COPY --from=builder /app/target/release/obsidian_agent /usr/local/bin/obsidian_agent
COPY --from=builder /app/target/release/llm_router_agent /usr/local/bin/llm_router_agent

# Copy configuration and static assets
COPY data/config.prod.toml ./data/config.toml
COPY frontend ./frontend

# Set environment variables
ENV APP_ENV=prod
ENV RUST_BACKTRACE=0
ENV RUST_LOG=info

# Container configuration
EXPOSE 8181
WORKDIR /app

# Set resource limits
ENV MALLOC_ARENA_MAX=2

# Configure health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD wget -q --spider http://localhost:8181/health || exit 1

# Set security settings: drop all capabilities except what we need
USER orchestrator

# Entrypoint with graceful shutdown
CMD ["master_orchestrator"]