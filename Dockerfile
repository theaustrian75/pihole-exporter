# syntax=docker/dockerfile:1

FROM rust:1-alpine3.24 AS builder

RUN apk upgrade --no-cache \
    && apk add --no-cache musl-dev git

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --locked --release --bin pihole-exporter

FROM alpine:3.24

LABEL org.opencontainers.image.description="Prometheus exporter for Pi-hole"

RUN apk upgrade --no-cache \
    && apk add --no-cache ca-certificates wget

COPY docker-healthcheck.sh /usr/local/bin/docker-healthcheck.sh
RUN chmod +x /usr/local/bin/docker-healthcheck.sh

WORKDIR /app/
COPY --from=builder /build/target/release/pihole-exporter ./pihole-exporter

EXPOSE 9617

# Timeout stays above HEALTH_CHECK_TIMEOUT (default 5s) so a live /healthz
# probe can finish before Docker kills this command.
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/docker-healthcheck.sh"]

USER 65532:65532

CMD ["./pihole-exporter"]
