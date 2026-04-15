#!/usr/bin/env bash
set -euo pipefail

pass() { echo "PASS: $1"; }
fail() { echo "FAIL: $1" >&2; exit 1; }

bool_is_true() {
  case "${1:-}" in
    true|1|yes|on) return 0 ;;
    *) return 1 ;;
  esac
}

require_env() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    fail "${name} is required"
  fi
}

append_optional_flag() {
  local -n out_ref="$1"
  local env_name="$2"
  local flag="$3"

  if [[ -n "${!env_name:-}" ]]; then
    out_ref+=("${flag}" "${!env_name}")
  fi
}

append_optional_bool_flag() {
  local -n out_ref="$1"
  local env_name="$2"
  local flag="$3"

  if bool_is_true "${!env_name:-}"; then
    out_ref+=("${flag}")
  fi
}

append_requested_scopes() {
  local -n out_ref="$1"
  local scopes="${PIR_GRAPH_BATCH_SCOPES:-graph,details}"
  local scope

  IFS=',' read -r -a raw_scopes <<< "${scopes}"
  for scope in "${raw_scopes[@]}"; do
    scope="${scope//[[:space:]]/}"
    if [[ -n "${scope}" ]]; then
      out_ref+=("--requested-scope" "${scope}")
    fi
  done
}

grpc_describe() {
  require_env KERNEL_GRPC_HOST
  require_env TLS_CA
  require_env TLS_CERT
  require_env TLS_KEY

  local kernel_port="${KERNEL_GRPC_PORT:-50054}"
  grpcurl \
    -cacert "${TLS_CA}" \
    -cert "${TLS_CERT}" \
    -key "${TLS_KEY}" \
    "${KERNEL_GRPC_HOST}:${kernel_port}" \
    describe 2>&1
}

build_graph_batch_roundtrip_command() {
  local -n out_ref="$1"
  local input_path="$2"
  local run_id="$3"
  local -a args

  require_env PIR_GRAPH_BATCH_NATS_URL
  require_env PIR_GRAPH_BATCH_GRPC_ENDPOINT

  args=(
    /usr/local/bin/graph_batch_roundtrip
    --input "${input_path}"
    --nats-url "${PIR_GRAPH_BATCH_NATS_URL}"
    --grpc-endpoint "${PIR_GRAPH_BATCH_GRPC_ENDPOINT}"
    --run-id "${run_id}"
    --role "${PIR_GRAPH_BATCH_ROLE:-incident-commander}"
  )

  append_requested_scopes args
  append_optional_flag args PIR_GRAPH_BATCH_DEPTH --depth
  append_optional_flag args PIR_GRAPH_BATCH_TOKEN_BUDGET --token-budget
  append_optional_flag args PIR_GRAPH_BATCH_REHYDRATION_MODE --rehydration-mode
  append_optional_flag args PIR_GRAPH_BATCH_DETAIL_NODE_ID --detail-node-id
  append_optional_flag args PIR_GRAPH_BATCH_WAIT_TIMEOUT_SECS --wait-timeout-secs
  append_optional_flag args PIR_GRAPH_BATCH_POLL_INTERVAL_MS --poll-interval-ms
  append_optional_flag args PIR_GRAPH_BATCH_GRPC_TLS_CA_PATH --grpc-tls-ca-path
  append_optional_flag args PIR_GRAPH_BATCH_GRPC_TLS_CERT_PATH --grpc-tls-cert-path
  append_optional_flag args PIR_GRAPH_BATCH_GRPC_TLS_KEY_PATH --grpc-tls-key-path
  append_optional_flag args PIR_GRAPH_BATCH_GRPC_TLS_DOMAIN_NAME --grpc-tls-domain-name
  append_optional_flag args PIR_GRAPH_BATCH_NATS_TLS_CA_PATH --nats-tls-ca-path
  append_optional_flag args PIR_GRAPH_BATCH_NATS_TLS_CERT_PATH --nats-tls-cert-path
  append_optional_flag args PIR_GRAPH_BATCH_NATS_TLS_KEY_PATH --nats-tls-key-path
  append_optional_bool_flag args PIR_GRAPH_BATCH_NATS_TLS_FIRST --nats-tls-first
  append_optional_bool_flag args PIR_GRAPH_BATCH_INCLUDE_RENDERED_CONTENT --include-rendered-content

  out_ref=("${args[@]}")
}

build_graph_relation_roundtrip_command() {
  local -n out_ref="$1"
  local input_path="$2"
  local run_id="$3"
  local -a args

  require_env PIR_GRAPH_BATCH_NATS_URL
  require_env PIR_GRAPH_BATCH_GRPC_ENDPOINT

  args=(
    /usr/local/bin/graph_relation_roundtrip
    --input "${input_path}"
    --nats-url "${PIR_GRAPH_BATCH_NATS_URL}"
    --grpc-endpoint "${PIR_GRAPH_BATCH_GRPC_ENDPOINT}"
    --run-id "${run_id}"
    --role "${PIR_GRAPH_BATCH_ROLE:-incident-commander}"
  )

  append_requested_scopes args
  append_optional_flag args PIR_GRAPH_BATCH_DEPTH --depth
  append_optional_flag args PIR_GRAPH_BATCH_TOKEN_BUDGET --token-budget
  append_optional_flag args PIR_GRAPH_BATCH_REHYDRATION_MODE --rehydration-mode
  append_optional_flag args PIR_GRAPH_BATCH_DETAIL_NODE_ID --detail-node-id
  append_optional_flag args PIR_GRAPH_BATCH_WAIT_TIMEOUT_SECS --wait-timeout-secs
  append_optional_flag args PIR_GRAPH_BATCH_POLL_INTERVAL_MS --poll-interval-ms
  append_optional_flag args PIR_GRAPH_BATCH_GRPC_TLS_CA_PATH --grpc-tls-ca-path
  append_optional_flag args PIR_GRAPH_BATCH_GRPC_TLS_CERT_PATH --grpc-tls-cert-path
  append_optional_flag args PIR_GRAPH_BATCH_GRPC_TLS_KEY_PATH --grpc-tls-key-path
  append_optional_flag args PIR_GRAPH_BATCH_GRPC_TLS_DOMAIN_NAME --grpc-tls-domain-name
  append_optional_flag args PIR_GRAPH_BATCH_NATS_TLS_CA_PATH --nats-tls-ca-path
  append_optional_flag args PIR_GRAPH_BATCH_NATS_TLS_CERT_PATH --nats-tls-cert-path
  append_optional_flag args PIR_GRAPH_BATCH_NATS_TLS_KEY_PATH --nats-tls-key-path
  append_optional_bool_flag args PIR_GRAPH_BATCH_NATS_TLS_FIRST --nats-tls-first
  append_optional_bool_flag args PIR_GRAPH_BATCH_INCLUDE_RENDERED_CONTENT --include-rendered-content

  out_ref=("${args[@]}")
}

build_graph_batch_vllm_request_command() {
  local -n out_ref="$1"
  local run_id="$2"
  local request_kind="${VLLM_REQUEST_KIND:-default}"
  local -a args

  args=(
    /usr/local/bin/graph_batch_vllm_request
    --run-id "${run_id}"
  )

  case "${request_kind}" in
    default) ;;
    blind) args+=(--blind) ;;
    large|large-incident) args+=(--large-incident) ;;
    *)
      fail "unsupported VLLM_REQUEST_KIND=${request_kind}"
      ;;
  esac

  if [[ -n "${VLLM_REQUEST_FIXTURE:-}" ]]; then
    args+=(--request-fixture "${VLLM_REQUEST_FIXTURE}")
  fi
  if [[ -n "${VLLM_REQUEST_SUBJECT_PREFIX:-}" ]]; then
    args+=(--subject-prefix "${VLLM_REQUEST_SUBJECT_PREFIX}")
  fi
  if bool_is_true "${VLLM_REQUEST_USE_REPAIR_JUDGE:-}"; then
    args+=(--use-repair-judge)
  fi
  if bool_is_true "${VLLM_REQUEST_USE_SEMANTIC_CLASSIFIER:-}"; then
    args+=(--use-semantic-classifier)
  fi
  if bool_is_true "${VLLM_REQUEST_NAMESPACE_NODE_IDS:-}"; then
    args+=(--namespace-node-ids)
  fi

  out_ref=("${args[@]}")
}

assert_roundtrip_summary() {
  local summary="$1"

  jq -e '
    .root_node_id != null and
    .published_messages > 0 and
    .neighbor_count >= 0 and
    .relationship_count >= 0 and
    .detail_count >= 0 and
    .rendered_chars >= 0
  ' >/dev/null <<<"${summary}" || fail "invalid graph_batch_roundtrip summary: ${summary}"
}
