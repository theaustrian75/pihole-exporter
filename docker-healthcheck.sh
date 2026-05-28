#!/bin/sh
set -e

listen="${BIND_ADDR:-0.0.0.0}:${PORT:-9617}"
port="${listen##*:}"
probe_url="http://127.0.0.1:${port}/healthz"
wget_extra=""

cert_file="${TLS_CERT_FILE:-}"
key_file="${TLS_KEY_FILE:-}"

if [ -n "$cert_file" ] && [ -n "$key_file" ] && [ -f "$cert_file" ] && [ -f "$key_file" ]; then
    probe_url="https://127.0.0.1:${port}/healthz"
    wget_extra="--no-check-certificate"
fi

# Omit wget -q so connection errors are written to stderr.
if ! wget $wget_extra -O- "$probe_url" >/dev/null; then
    echo "healthcheck failed: GET $probe_url" >&2
    exit 1
fi
