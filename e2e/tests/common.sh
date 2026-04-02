#!/usr/bin/env bash
# Shared helpers for kernel E2E tests. Source this file.
set -euo pipefail

: "${KERNEL_HOST:?KERNEL_HOST is required}"
: "${KERNEL_PORT:?KERNEL_PORT is required}"
: "${TLS_CA:?TLS_CA is required}"
: "${TLS_CERT:?TLS_CERT is required}"
: "${TLS_KEY:?TLS_KEY is required}"

KERNEL_ADDR="${KERNEL_HOST}:${KERNEL_PORT}"

grpc_call() {
  local service="$1"
  local method="$2"
  local payload="${3:-{}}"

  grpcurl \
    -cacert "${TLS_CA}" \
    -cert "${TLS_CERT}" \
    -key "${TLS_KEY}" \
    -d "${payload}" \
    "${KERNEL_ADDR}" \
    "${service}/${method}"
}

pass() { echo "PASS: $1"; }
fail() { echo "FAIL: $1" >&2; exit 1; }
