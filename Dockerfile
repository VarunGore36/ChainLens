FROM rust:1.85-slim as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY migrations/ migrations/
COPY benches/ benches/

RUN cargo build --release --bin chainlens --bin api

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/chainlens ./
COPY --from=builder /app/target/release/api ./
COPY --from=builder /app/migrations/ ./migrations/

EXPOSE 8080

CMD ["./api"]