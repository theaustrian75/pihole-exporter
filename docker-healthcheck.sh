#!/bin/sh
set -e

port="${PORT:-9617}"
probe_url="http://127.0.0.1:${port}/liveness"
wget_extra=""

if [ -n "${TLS_CERT_FILE:-}" ] && [ -n "${TLS_KEY_FILE:-}" ] \
    && [ -f "$TLS_CERT_FILE" ] && [ -f "$TLS_KEY_FILE" ]; then
    probe_url="https://127.0.0.1:${port}/liveness"
    wget_extra="--no-check-certificate"
fi

# Omit wget -q so connection errors are written to stderr.
if ! wget $wget_extra -O- "$probe_url" >/dev/null; then
    echo "healthcheck failed: GET $probe_url" >&2
    exit 1
fi
