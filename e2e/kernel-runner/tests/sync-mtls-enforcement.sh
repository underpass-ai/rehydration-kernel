#!/usr/bin/env bash
set -euo pipefail
source /app/common.sh

echo "=== sync-mtls-enforcement ==="

require_env KERNEL_GRPC_HOST
require_env TLS_CA
require_env TLS_CERT
require_env TLS_KEY

kernel_port="${KERNEL_GRPC_PORT:-50054}"
kernel_addr="${KERNEL_GRPC_HOST}:${kernel_port}"

set +e
output_auth="$(grpcurl \
  -cacert "${TLS_CA}" \
  -cert "${TLS_CERT}" \
  -key "${TLS_KEY}" \
  -connect-timeout 5 \
  "${kernel_addr}" \
  describe 2>&1)"
auth_status=$?
set -e

if echo "${output_auth}" | grep -q "reflection API"; then
  :
elif echo "${output_auth}" | grep -qi "tls.*fail\|certificate.*reject\|connection refused"; then
  fail "authenticated call rejected at TLS: ${output_auth}"
elif [[ ${auth_status} -ne 0 ]]; then
  fail "authenticated call failed: ${output_auth}"
fi

output_anon="$(grpcurl \
  -cacert "${TLS_CA}" \
  -connect-timeout 5 \
  "${kernel_addr}" \
  describe 2>&1)" || true

if echo "${output_anon}" | grep -qi "tls\|certificate\|handshake\|reset\|refused\|EOF\|transport\|deadline\|timeout\|context"; then
  pass "anonymous call rejected (${output_anon##*: })"
else
  fail "anonymous call not rejected: ${output_anon}"
fi
