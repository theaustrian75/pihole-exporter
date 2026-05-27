ARG IMAGE=alpine:3.21

FROM rust:1.88-alpine3.21 AS builder

WORKDIR /build
RUN apk --no-cache add musl-dev git

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM $IMAGE

RUN apk --no-cache add ca-certificates wget

LABEL name="pihole-exporter"

WORKDIR /app/
COPY --from=builder /build/target/release/pihole-exporter ./pihole-exporter

EXPOSE 9617

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD wget -qO- http://127.0.0.1:9617/healthz >/dev/null || exit 1

USER 65532:65532

CMD ["./pihole-exporter"]
