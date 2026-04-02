#!/usr/bin/env bash
# Test 04: mTLS enforcement — anonymous calls must be rejected.
set -euo pipefail
source "$(dirname "$0")/common.sh"

echo "=== Test 04: mTLS Enforcement ==="

# Authenticated: must succeed at TLS level.
output_auth=$(grpcurl \
  -cacert "${TLS_CA}" \
  -cert "${TLS_CERT}" \
  -key "${TLS_KEY}" \
  -connect-timeout 5 \
  "${KERNEL_ADDR}" \
  describe 2>&1) || true

if echo "${output_auth}" | grep -qi "tls.*fail\|certificate.*reject\|connection refused"; then
  fail "Authenticated call rejected at TLS: ${output_auth}"
fi
pass "Authenticated mTLS call accepted"

# Anonymous (no client cert): must fail (TLS error, timeout, or reset).
output_anon=$(grpcurl \
  -cacert "${TLS_CA}" \
  -connect-timeout 5 \
  "${KERNEL_ADDR}" \
  describe 2>&1) || true

if echo "${output_anon}" | grep -qi "tls\|certificate\|handshake\|reset\|refused\|EOF\|transport\|deadline\|timeout\|context"; then
  pass "Anonymous call rejected (${output_anon##*: })"
else
  fail "Anonymous call not rejected: ${output_anon}"
fi

pass "mTLS enforcement verified"
