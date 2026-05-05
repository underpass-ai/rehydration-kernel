#!/usr/bin/env bash
set -euo pipefail
source /app/common.sh

echo "=== sync-kernel-memory-service ==="

memory_service="underpass.rehydration.kernel.v1beta1.KernelMemoryService"
memory_proto="underpass/rehydration/kernel/v1beta1/memory.proto"
proto_root="${KERNEL_PROTO_ROOT:-/app/api/proto}"
kernel_addr="$(kernel_grpc_addr)"
run_id="${RUN_ID:-helm-kms-$(date +%s)-$$}"
dimension="timeline-${run_id}"
about_a="e2e:kms:${run_id}:a"
about_b="e2e:kms:${run_id}:b"
source_ref="claim:${run_id}:source"
target_ref="claim:${run_id}:target"
conflict_ref="claim:${run_id}:conflict"
other_ref="claim:${run_id}:other"
evidence_ref="evidence:${run_id}:target"

grpc_memory_call() {
  local method="$1"
  local payload="$2"
  local -a transport_args=()
  append_grpcurl_transport_args transport_args

  grpcurl \
    "${transport_args[@]}" \
    -import-path "${proto_root}" \
    -proto "${memory_proto}" \
    -d "${payload}" \
    "${kernel_addr}" \
    "${memory_service}/${method}"
}

assert_jq() {
  local payload="$1"
  shift
  local -a jq_args=("$@")
  local last_index=$((${#jq_args[@]} - 1))
  local message="${jq_args[${last_index}]}"
  unset "jq_args[${last_index}]"

  jq -e "${jq_args[@]}" >/dev/null <<<"${payload}" || fail "${message}: ${payload}"
}

ingest_a_payload="$(
  jq -nc \
    --arg about "${about_a}" \
    --arg dimension "${dimension}" \
    --arg source_ref "${source_ref}" \
    --arg target_ref "${target_ref}" \
    --arg conflict_ref "${conflict_ref}" \
    --arg evidence_ref "${evidence_ref}" \
    --arg run_id "${run_id}" \
    '{
      about: $about,
      memory: {
        dimensions: [
          {
            id: $dimension,
            kind: "conversation",
            title: ("KernelMemoryService E2E " + $run_id)
          }
        ],
        entries: [
          {
            id: $source_ref,
            kind: "claim",
            text: "Source claim for KernelMemoryService Helm e2e.",
            coordinates: [
              {
                dimension: $dimension,
                scopeId: $dimension,
                sequence: 1
              }
            ]
          },
          {
            id: $target_ref,
            kind: "claim",
            text: "Target claim for KernelMemoryService Helm e2e.",
            coordinates: [
              {
                dimension: $dimension,
                scopeId: $dimension,
                sequence: 2
              }
            ]
          },
          {
            id: $conflict_ref,
            kind: "claim",
            text: "Conflicting claim for KernelMemoryService Helm e2e.",
            coordinates: [
              {
                dimension: $dimension,
                scopeId: $dimension,
                sequence: 3
              }
            ]
          }
        ],
        relations: [
          {
            sourceRef: $source_ref,
            targetRef: $target_ref,
            rel: "supports",
            semanticClass: "MEMORY_SEMANTIC_CLASS_EVIDENTIAL",
            why: "source supports target for KernelMemoryService Helm e2e",
            confidence: "MEMORY_CONFIDENCE_HIGH",
            sequence: 10
          },
          {
            sourceRef: $target_ref,
            targetRef: $conflict_ref,
            rel: "contradicts",
            semanticClass: "MEMORY_SEMANTIC_CLASS_EVIDENTIAL",
            why: "target and conflict cannot both be true for KernelMemoryService Helm e2e",
            confidence: "MEMORY_CONFIDENCE_HIGH",
            sequence: 11
          }
        ],
        evidence: [
          {
            id: $evidence_ref,
            supports: [$target_ref],
            text: "Evidence for KernelMemoryService Helm e2e target.",
            source: "helm-e2e"
          }
        ]
      },
      provenance: {
        sourceKind: "MEMORY_SOURCE_KIND_AGENT",
        sourceAgent: "helm-e2e",
        observedAt: "2026-05-04T00:00:00Z"
      },
      idempotencyKey: ("kernel-memory-service-a-" + $run_id)
    }'
)"

ingest_b_payload="$(
  jq -nc \
    --arg about "${about_b}" \
    --arg dimension "${dimension}" \
    --arg other_ref "${other_ref}" \
    --arg run_id "${run_id}" \
    '{
      about: $about,
      memory: {
        dimensions: [
          {
            id: $dimension,
            kind: "conversation",
            title: ("KernelMemoryService E2E second about " + $run_id)
          }
        ],
        entries: [
          {
            id: $other_ref,
            kind: "claim",
            text: "Second about claim for ALL_ABOUTS Helm e2e.",
            coordinates: [
              {
                dimension: $dimension,
                scopeId: $dimension,
                sequence: 3
              }
            ]
          }
        ],
        relations: [],
        evidence: []
      },
      provenance: {
        sourceKind: "MEMORY_SOURCE_KIND_AGENT",
        sourceAgent: "helm-e2e",
        observedAt: "2026-05-04T00:00:00Z"
      },
      idempotencyKey: ("kernel-memory-service-b-" + $run_id)
    }'
)"

ingest_a="$(grpc_memory_call Ingest "${ingest_a_payload}")"
assert_jq "${ingest_a}" \
  '.memory.readAfterWriteReady == true and .memory.accepted.entries == 3 and .memory.accepted.relations == 2 and .memory.accepted.evidence == 1' \
  "Ingest A did not accept expected memory"

ingest_b="$(grpc_memory_call Ingest "${ingest_b_payload}")"
assert_jq "${ingest_b}" \
  '.memory.readAfterWriteReady == true and
   .memory.accepted.entries == 1 and
   (.memory.accepted.relations // 0) == 0 and
   (.memory.accepted.evidence // 0) == 0' \
  "Ingest B did not accept expected memory"

dimension_selection_current="$(
  jq -nc --arg dimension "${dimension}" '{
    mode: "DIMENSION_SELECTION_MODE_ONLY",
    include: [$dimension]
  }'
)"

wake="$(grpc_memory_call Wake "$(
  jq -nc --arg about "${about_a}" --argjson dimensions "${dimension_selection_current}" '{
    about: $about,
    role: "helm-e2e",
    intent: "verify KernelMemoryService wake",
    budget: {
      tokens: 1600,
      detail: "MEMORY_DETAIL_LEVEL_FULL",
      depth: 3
    },
    dimensions: $dimensions
  }'
)")"
assert_jq "${wake}" '.summary | length > 0' "Wake did not return summary"

ask="$(grpc_memory_call Ask "$(
  jq -nc --arg about "${about_a}" --argjson dimensions "${dimension_selection_current}" '{
    about: $about,
    question: "What supports the target claim?",
    answerPolicy: "ANSWER_POLICY_BEST_EFFORT",
    budget: {
      tokens: 1600,
      detail: "MEMORY_DETAIL_LEVEL_FULL",
      depth: 3
    },
    dimensions: $dimensions
  }'
)")"
assert_jq "${ask}" \
  --arg expected_ref "detail:${evidence_ref}" \
  '.answer == "Evidence for KernelMemoryService Helm e2e target." and
   (.because | length) == 1 and
   .because[0].ref == $expected_ref' \
  "Ask did not return deterministic evidence text"

ask_conflicts="$(grpc_memory_call Ask "$(
  jq -nc --arg about "${about_a}" --argjson dimensions "${dimension_selection_current}" '{
    about: $about,
    question: "Which claims conflict?",
    answerPolicy: "ANSWER_POLICY_SHOW_CONFLICTS",
    budget: {
      tokens: 1600,
      detail: "MEMORY_DETAIL_LEVEL_FULL",
      depth: 3
    },
    dimensions: $dimensions
  }'
)")"
assert_jq "${ask_conflicts}" \
  --arg target_ref "${target_ref}" \
  --arg conflict_ref "${conflict_ref}" \
  '.proof.conflicts | any(contains($target_ref) and contains("contradicts") and contains($conflict_ref))' \
  "Ask show_conflicts did not surface explicit conflict relation"

goto="$(grpc_memory_call Goto "$(
  jq -nc --arg about "${about_a}" --arg dimension "${dimension}" '{
    about: $about,
    cursor: { sequence: 2 },
    dimensions: {
      mode: "DIMENSION_SELECTION_MODE_ONLY",
      include: [$dimension]
    },
    limit: { entries: 5 },
    include: { evidence: true, relations: true },
    budget: { tokens: 1600, depth: 3 }
  }'
)")"
assert_jq "${goto}" \
  --arg target_ref "${target_ref}" \
  '.entries | map(.ref) | index($target_ref) != null' \
  "Goto did not resolve target entry"

near="$(grpc_memory_call Near "$(
  jq -nc --arg about "${about_a}" --arg target_ref "${target_ref}" --arg dimension "${dimension}" '{
    about: $about,
    around: { ref: $target_ref },
    dimensions: {
      mode: "DIMENSION_SELECTION_MODE_ONLY",
      include: [$dimension]
    },
    window: { beforeEntries: 1, afterEntries: 1 },
    limit: { entries: 5 },
    include: { evidence: true, relations: true },
    budget: { tokens: 1600, depth: 3 }
  }'
)")"
assert_jq "${near}" \
  --arg source_ref "${source_ref}" \
  --arg target_ref "${target_ref}" \
  '(.temporal.resolved.sequence == 2) and
   (.entries | map(.ref) | index($source_ref) != null) and
   (.proof.path | any(.sourceRef == $source_ref and .targetRef == $target_ref and .rel == "supports"))' \
  "Near did not return source neighbor and target proof"

rewind_current="$(grpc_memory_call Rewind "$(
  jq -nc --arg about "${about_a}" --arg dimension "${dimension}" '{
    about: $about,
    cursor: { sequence: 4 },
    dimensions: {
      mode: "DIMENSION_SELECTION_MODE_ONLY",
      include: [$dimension]
    },
    limit: { entries: 10 },
    include: { evidence: true, relations: true },
    budget: { tokens: 1600, depth: 3 }
  }'
)")"
assert_jq "${rewind_current}" \
  --arg other_ref "${other_ref}" \
  '(.entries | length) == 3 and (.entries | map(.ref) | index($other_ref) == null)' \
  "Rewind current about leaked entries from another about"

rewind_all="$(grpc_memory_call Rewind "$(
  jq -nc --arg about "${about_a}" --arg dimension "${dimension}" '{
    about: $about,
    cursor: { sequence: 4 },
    dimensions: {
      mode: "DIMENSION_SELECTION_MODE_ONLY",
      include: [$dimension],
      scope: "DIMENSION_SCOPE_MODE_ALL_ABOUTS"
    },
    limit: { entries: 10 },
    include: { evidence: true, relations: true },
    budget: { tokens: 1600, depth: 3 }
  }'
)")"
assert_jq "${rewind_all}" \
  --arg source_ref "${source_ref}" \
  --arg target_ref "${target_ref}" \
  --arg other_ref "${other_ref}" \
  '(.coverage.requested.scope == "DIMENSION_SCOPE_MODE_ALL_ABOUTS") and
   (.entries | map(.ref) | index($source_ref) != null) and
   (.entries | map(.ref) | index($target_ref) != null) and
   (.entries | map(.ref) | index($other_ref) != null)' \
  "ALL_ABOUTS rewind did not include both abouts"

forward="$(grpc_memory_call Forward "$(
  jq -nc --arg about "${about_a}" --arg source_ref "${source_ref}" --arg dimension "${dimension}" '{
    about: $about,
    cursor: { ref: $source_ref },
    dimensions: {
      mode: "DIMENSION_SELECTION_MODE_ONLY",
      include: [$dimension]
    },
    limit: { entries: 5 },
    include: { evidence: true, relations: true },
    budget: { tokens: 1600, depth: 3 }
  }'
)")"
assert_jq "${forward}" \
  --arg target_ref "${target_ref}" \
  '.entries | map(.ref) | index($target_ref) != null' \
  "Forward did not include target entry"

temporal_raw="$(grpc_memory_call Goto "$(
  jq -nc --arg about "${about_a}" --arg dimension "${dimension}" '{
    about: $about,
    cursor: { sequence: 2 },
    dimensions: {
      mode: "DIMENSION_SELECTION_MODE_ONLY",
      include: [$dimension]
    },
    include: { rawRefs: true },
    budget: { tokens: 1600, depth: 3 }
  }'
)")"
assert_jq "${temporal_raw}" \
  --arg target_ref "${target_ref}" \
  '(.rawRefs | length) >= 1 and
   (.rawRefs | any(.ref == $target_ref and (.coordinates | length) >= 1))' \
  "Temporal rawRefs=true did not return typed raw refs"

trace="$(grpc_memory_call Trace "$(
  jq -nc --arg source_ref "${source_ref}" --arg target_ref "${target_ref}" '{
    from: $source_ref,
    to: $target_ref,
    goal: "verify direct supports relation",
    budget: { tokens: 1600, depth: 3 }
  }'
)")"
assert_jq "${trace}" \
  --arg source_ref "${source_ref}" \
  --arg target_ref "${target_ref}" \
  '.trace | any(.sourceRef == $source_ref and .targetRef == $target_ref and .rel == "supports")' \
  "Trace did not return direct supports relation"

inspect_target="$(grpc_memory_call Inspect "$(
  jq -nc --arg target_ref "${target_ref}" '{
    ref: $target_ref,
    include: {
      incoming: true,
      outgoing: true,
      details: true,
      raw: true
    }
  }'
)")"
assert_jq "${inspect_target}" \
  --arg source_ref "${source_ref}" \
  --arg target_ref "${target_ref}" \
  '.object.ref == $target_ref and
   (.links.incoming // [] | any(.sourceRef == $source_ref and .targetRef == $target_ref and .rel == "supports")) and
   (.evidence | length) >= 1 and
   (.raw | any(.ref == $target_ref and (.detail | length) > 0))' \
  "Inspect target did not return incoming supports link, detail evidence, and typed raw audit ref"

inspect_source="$(grpc_memory_call Inspect "$(
  jq -nc --arg source_ref "${source_ref}" '{
    ref: $source_ref,
    include: {
      incoming: true,
      outgoing: true,
      details: true,
      raw: false
    }
  }'
)")"
assert_jq "${inspect_source}" \
  --arg source_ref "${source_ref}" \
  --arg target_ref "${target_ref}" \
  '.object.ref == $source_ref and
   (.links.outgoing // [] | any(.sourceRef == $source_ref and .targetRef == $target_ref and .rel == "supports"))' \
  "Inspect source did not return outgoing supports link"

pass "KernelMemoryService typed gRPC lifecycle passed for ${run_id}"
