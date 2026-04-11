#!/usr/bin/env bash
set -euo pipefail
source /app/common.sh

echo "=== async-graph-batch-roundtrip ==="

run_id="${RUN_ID:-e2e-async-graph-$(date +%s)}"
input_path="${GRAPH_BATCH_INPUT:-/app/api/examples/kernel/v1beta1/async/incident-graph-batch.json}"

[[ -f "${input_path}" ]] || fail "GraphBatch fixture not found: ${input_path}"

roundtrip_cmd=()
build_graph_batch_roundtrip_command roundtrip_cmd "${input_path}" "${run_id}"

summary="$("${roundtrip_cmd[@]}")" || fail "graph_batch_roundtrip command failed"
assert_roundtrip_summary "${summary}"

root_node_id="$(jq -r '.root_node_id' <<<"${summary}")"
published_messages="$(jq -r '.published_messages' <<<"${summary}")"
relationship_count="$(jq -r '.relationship_count' <<<"${summary}")"
detail_count="$(jq -r '.detail_count' <<<"${summary}")"

echo "${summary}" | jq .
pass "async roundtrip succeeded root=${root_node_id} published_messages=${published_messages} relationships=${relationship_count} details=${detail_count}"
