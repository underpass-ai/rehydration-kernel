#!/usr/bin/env bash
set -euo pipefail
source /app/common.sh

echo "=== sync-grpc-handshake ==="

set +e
output="$(grpc_describe 2>&1)"
status=$?
set -e

if echo "${output}" | grep -q "reflection API"; then
  pass "mTLS handshake succeeded (gRPC server reachable, reflection not enabled)"
elif echo "${output}" | grep -qi "tls\|certificate\|handshake\|connection refused"; then
  fail "TLS handshake failed: ${output}"
elif [[ ${status} -eq 0 ]]; then
  pass "gRPC server responded: ${output}"
else
  fail "${output}"
fi
