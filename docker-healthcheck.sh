#!/bin/sh
# Probe the exporter's live /healthz endpoint.
# The in-process check is bounded by HEALTH_CHECK_TIMEOUT (default 5s).
# The image HEALTHCHECK timeout is 10s so this script can observe the result.
set -eu

listen="${BIND_ADDR:-0.0.0.0}:${PORT:-9617}"
port="${listen##*:}"
probe_url="http://127.0.0.1:${port}/healthz"

cert_file="${TLS_CERT_FILE:-}"
key_file="${TLS_KEY_FILE:-}"
use_tls=0

if [ -n "$cert_file" ] && [ -n "$key_file" ] && [ -f "$cert_file" ] && [ -f "$key_file" ]; then
    probe_url="https://127.0.0.1:${port}/healthz"
    use_tls=1
fi

probe() {
    if [ "$use_tls" -eq 1 ]; then
        wget --no-check-certificate -O- "$probe_url"
    else
        wget -O- "$probe_url"
    fi
}

# Keep a healthy probe quiet. On failure, include wget's error (connection
# refused, TLS, HTTP 503) instead of only a generic exit status.
if ! output=$(probe 2>&1 >/dev/null); then
    echo "healthcheck failed: GET $probe_url" >&2
    printf '%s\n' "$output" >&2
    exit 1
fi
