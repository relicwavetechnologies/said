# Multi-stage build for the Voice Polish control-plane service
# Stage 1: build
FROM rust:1.82-slim AS builder
WORKDIR /app

# Cache dependencies first
COPY Cargo.toml .
RUN mkdir src && echo 'fn main(){}' > src/main.rs && cargo build --release 2>/dev/null || true

# Build the real binary
COPY . .
RUN touch src/main.rs && cargo build --release

# Stage 2: minimal runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/control-plane /usr/local/bin/control-plane

ENV PORT=8080
EXPOSE 8080

CMD ["control-plane", "--port", "8080"]
