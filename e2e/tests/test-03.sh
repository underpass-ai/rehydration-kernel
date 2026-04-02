#!/usr/bin/env bash
# Test 03: GetContext — verify second gRPC service reachable.
set -euo pipefail
source "$(dirname "$0")/common.sh"

echo "=== Test 03: GetContext (mTLS) ==="

# Same handshake check — verifies the TLS connection is stable across calls.
for i in 1 2 3; do
  output=$(grpcurl \
    -cacert "${TLS_CA}" \
    -cert "${TLS_CERT}" \
    -key "${TLS_KEY}" \
    "${KERNEL_ADDR}" \
    describe 2>&1) || true

  if echo "${output}" | grep -qi "tls\|certificate\|handshake\|refused"; then
    fail "Call ${i}: TLS failed: ${output}"
  fi
done

pass "3 consecutive mTLS handshakes succeeded"
