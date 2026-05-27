# syntax=docker/dockerfile:1

FROM rust:1-alpine3.23 AS builder

RUN apk add --no-cache musl-dev git

WORKDIR /build

COPY Cargo.toml ./
COPY src ./src

ARG TARGETARCH=amd64
RUN set -eu; \
    case "${TARGETARCH}" in \
        amd64) RUST_TARGET=x86_64-unknown-linux-musl ;; \
        arm64) RUST_TARGET=aarch64-unknown-linux-musl ;; \
        *) echo "unsupported architecture: ${TARGETARCH}" >&2; exit 1 ;; \
    esac; \
    rustup target add "${RUST_TARGET}"; \
    cargo build --release --target "${RUST_TARGET}"; \
    install -Dm755 "/build/target/${RUST_TARGET}/release/pihole-exporter" /build/pihole-exporter

FROM alpine:3.23

RUN apk add --no-cache ca-certificates wget

LABEL org.opencontainers.image.description="Prometheus exporter for Pi-hole"

WORKDIR /app/
COPY --from=builder /build/pihole-exporter ./pihole-exporter

EXPOSE 9617

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD wget -qO- http://127.0.0.1:9617/healthz >/dev/null || exit 1

USER 65532:65532

CMD ["./pihole-exporter"]
