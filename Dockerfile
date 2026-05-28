# syntax=docker/dockerfile:1

FROM rust:1-alpine3.23 AS builder

RUN apk add --no-cache musl-dev git

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --locked --release --bin pihole-exporter

FROM alpine:3.23

LABEL org.opencontainers.image.description="Prometheus exporter for Pi-hole"

RUN apk add --no-cache ca-certificates wget

WORKDIR /app/
COPY --from=builder /build/target/release/pihole-exporter ./pihole-exporter

EXPOSE 9617

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD wget -qO- http://127.0.0.1:9617/healthz >/dev/null || exit 1

USER 65532:65532

CMD ["./pihole-exporter"]
