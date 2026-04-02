#!/usr/bin/env bash
# Test 01: Health — verify mTLS handshake succeeds against kernel gRPC.
set -euo pipefail
source "$(dirname "$0")/common.sh"

echo "=== Test 01: Health (mTLS handshake) ==="

# grpcurl describe without reflection fails but the TLS handshake must succeed.
# A TLS error = transport failure. A "reflection API" error = transport OK.
output=$(grpcurl \
  -cacert "${TLS_CA}" \
  -cert "${TLS_CERT}" \
  -key "${TLS_KEY}" \
  "${KERNEL_ADDR}" \
  describe 2>&1) || true

if echo "${output}" | grep -q "reflection API"; then
  pass "mTLS handshake succeeded (gRPC server reachable, reflection not enabled)"
elif echo "${output}" | grep -qi "tls\|certificate\|handshake\|connection refused"; then
  fail "TLS handshake failed: ${output}"
else
  pass "gRPC server responded: ${output}"
fi
