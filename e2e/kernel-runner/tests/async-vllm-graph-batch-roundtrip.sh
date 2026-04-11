#!/usr/bin/env bash
set -euo pipefail
source /app/common.sh

echo "=== async-vllm-graph-batch-roundtrip ==="

require_env LLM_ENDPOINT
require_env LLM_MODEL
require_env LLM_PROVIDER

run_id="${RUN_ID:-e2e-vllm-graph-$(date +%s)}"

request_cmd=()
build_graph_batch_vllm_request_command request_cmd "${run_id}"

roundtrip_cmd=()
build_graph_batch_roundtrip_command roundtrip_cmd "-" "${run_id}"

summary="$("${request_cmd[@]}" | "${roundtrip_cmd[@]}")" || fail "vLLM GraphBatch roundtrip pipeline failed"
assert_roundtrip_summary "${summary}"

root_node_id="$(jq -r '.root_node_id' <<<"${summary}")"
published_messages="$(jq -r '.published_messages' <<<"${summary}")"
rendered_chars="$(jq -r '.rendered_chars' <<<"${summary}")"

echo "${summary}" | jq .
pass "async model-driven roundtrip succeeded root=${root_node_id} published_messages=${published_messages} rendered_chars=${rendered_chars}"
