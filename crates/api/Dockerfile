# Multi-stage build for Rust API with cargo-chef
FROM rust:1.94-slim AS chef
RUN apt-get update && apt-get install -y curl build-essential gcc-x86-64-linux-gnu pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-gnu
RUN cargo install cargo-chef
WORKDIR /app

# Planner stage - analyze dependencies
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage - cache dependencies
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies only (this layer is cached)
ENV CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc
RUN cargo chef cook --release --recipe-path recipe.json --target x86_64-unknown-linux-gnu

# Copy source and build application
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

ENV CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc
RUN cargo build --release --target x86_64-unknown-linux-gnu --bin readceipt_api

# Runtime stage
FROM cgr.dev/chainguard/glibc-dynamic

WORKDIR /app
COPY --from=builder /app/target/x86_64-unknown-linux-gnu/release/readceipt_api ./api
COPY config ./config

EXPOSE 8080
CMD ["./api"]
