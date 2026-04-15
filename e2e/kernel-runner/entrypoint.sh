#!/usr/bin/env bash
set -euo pipefail

if [[ $# -gt 0 ]]; then
  case "$1" in
    --list|list)
      test_id="list"
      ;;
    *)
      test_id="$1"
      shift
      ;;
  esac
else
  test_id="${TEST_ID:-sync-grpc-handshake}"
fi

case "${test_id}" in
  01|sync-grpc-handshake)
    script="/app/tests/sync-grpc-handshake.sh"
    ;;
  02|sync-mtls-enforcement)
    script="/app/tests/sync-mtls-enforcement.sh"
    ;;
  11|async-graph-batch-roundtrip)
    script="/app/tests/async-graph-batch-roundtrip.sh"
    ;;
  14|async-graph-relation-roundtrip)
    script="/app/tests/async-graph-relation-roundtrip.sh"
    ;;
  15|async-vllm-spine-ab-comparison)
    script="/app/tests/async-vllm-spine-ab-comparison.sh"
    ;;
  12|async-vllm-graph-batch-roundtrip)
    script="/app/tests/async-vllm-graph-batch-roundtrip.sh"
    ;;
  13|async-vllm-blind-context-consumption)
    script="/app/tests/async-vllm-blind-context-consumption.sh"
    ;;
  list|--list)
    cat <<'EOF'
Supported TEST_ID values:
  01 | sync-grpc-handshake
  02 | sync-mtls-enforcement
  11 | async-graph-batch-roundtrip
  14 | async-graph-relation-roundtrip
  15 | async-vllm-spine-ab-comparison
  12 | async-vllm-graph-batch-roundtrip
  13 | async-vllm-blind-context-consumption
EOF
    exit 0
    ;;
  *)
    echo "unknown TEST_ID=${test_id}" >&2
    cat <<'EOF' >&2
Supported TEST_ID values:
  01 | sync-grpc-handshake
  02 | sync-mtls-enforcement
  11 | async-graph-batch-roundtrip
  14 | async-graph-relation-roundtrip
  15 | async-vllm-spine-ab-comparison
  12 | async-vllm-graph-batch-roundtrip
  13 | async-vllm-blind-context-consumption
EOF
    exit 2
    ;;
esac

exec "${script}" "$@"
