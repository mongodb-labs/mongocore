# Build stage
FROM rust:1.78-slim AS builder

RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY proto/ proto/
COPY build.rs ./

RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/mongocore /usr/local/bin/mongocore

EXPOSE 50051 3000

ENTRYPOINT ["mongocore"]
