#!/usr/bin/env bash

set -euo pipefail

ACTION="${1:-run}"
KERNEL_NAMESPACE="${KERNEL_NAMESPACE:-underpass-runtime}"
GRAPH_NAMESPACE="${GRAPH_NAMESPACE:-swe-ai-fleet}"
KERNEL_SERVICE="${KERNEL_SERVICE:-rehydration-kernel}"
NATS_SERVICE="${NATS_SERVICE:-nats}"
NEO4J_POD="${NEO4J_POD:-neo4j-0}"
LOCAL_GRPC_PORT="${LOCAL_GRPC_PORT:-50054}"
LOCAL_NATS_PORT="${LOCAL_NATS_PORT:-4222}"
SUBJECT_PREFIX="${SUBJECT_PREFIX:-rehydration}"
AUTO_CLEANUP="${AUTO_CLEANUP:-true}"

ROOT_NODE_ID="incident:starship-odyssey:port-manifold-breach"
STARSHIP_NODE_IDS=(
  "incident:starship-odyssey:port-manifold-breach"
  "decision:reroute-reserve-power"
  "decision:delay-jump-window"
  "decision:isolate-docking-ring"
  "decision:manual-throttle-guard"
  "task:stabilize-port-manifold"
  "task:reroute-eps-grid"
  "task:calibrate-nav-drift"
  "task:seal-docking-ring-twelve"
  "task:stage-medical-response"
  "task:validate-telemetry-mirror"
  "subsystem:propulsion"
  "subsystem:navigation"
  "subsystem:life-support"
  "crew:chief-engineer"
)

TMP_DIR="$(mktemp -d)"
GRPC_FORWARD_PID=""
NATS_FORWARD_PID=""

cleanup_port_forwards() {
  if [[ -n "${GRPC_FORWARD_PID}" ]]; then
    kill "${GRPC_FORWARD_PID}" >/dev/null 2>&1 || true
    wait "${GRPC_FORWARD_PID}" >/dev/null 2>&1 || true
  fi
  if [[ -n "${NATS_FORWARD_PID}" ]]; then
    kill "${NATS_FORWARD_PID}" >/dev/null 2>&1 || true
    wait "${NATS_FORWARD_PID}" >/dev/null 2>&1 || true
  fi
  rm -rf "${TMP_DIR}"
}

trap cleanup_port_forwards EXIT

bool_is_true() {
  [[ "${1}" == "true" || "${1}" == "1" || "${1}" == "yes" ]]
}

wait_for_local_port() {
  local port="$1"

  for _ in $(seq 1 60); do
    if bash -lc ">/dev/tcp/127.0.0.1/${port}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done

  echo "local port ${port} did not become ready" >&2
  return 1
}

find_valkey_pod() {
  kubectl get pods -n "${KERNEL_NAMESPACE}" -o name \
    | grep '^pod/valkey-' \
    | head -n 1 \
    | cut -d/ -f2
}

start_port_forwards() {
  kubectl port-forward -n "${KERNEL_NAMESPACE}" "svc/${KERNEL_SERVICE}" "${LOCAL_GRPC_PORT}:50054" \
    >"${TMP_DIR}/grpc-port-forward.log" 2>&1 &
  GRPC_FORWARD_PID="$!"

  kubectl port-forward -n "${KERNEL_NAMESPACE}" "svc/${NATS_SERVICE}" "${LOCAL_NATS_PORT}:4222" \
    >"${TMP_DIR}/nats-port-forward.log" 2>&1 &
  NATS_FORWARD_PID="$!"

  wait_for_local_port "${LOCAL_GRPC_PORT}"
  wait_for_local_port "${LOCAL_NATS_PORT}"
}

cleanup_starship_data() {
  local valkey_pod
  local -a redis_keys
  local cypher_ids

  valkey_pod="$(find_valkey_pod)"
  if [[ -z "${valkey_pod}" ]]; then
    echo "could not find a valkey pod in ${KERNEL_NAMESPACE}" >&2
    exit 1
  fi

  redis_keys=(
    "rehydration:snapshot:${ROOT_NODE_ID}:developer"
    "rehydration:snapshot:${ROOT_NODE_ID}:reviewer"
  )
  for node_id in "${STARSHIP_NODE_IDS[@]}"; do
    redis_keys+=("rehydration:node-detail:${node_id}")
  done

  kubectl exec -n "${KERNEL_NAMESPACE}" "${valkey_pod}" -- \
    redis-cli DEL "${redis_keys[@]}" >/dev/null

  cypher_ids="'${STARSHIP_NODE_IDS[0]}'"
  for node_id in "${STARSHIP_NODE_IDS[@]:1}"; do
    cypher_ids="${cypher_ids}, '${node_id}'"
  done

  kubectl exec -n "${GRAPH_NAMESPACE}" "${NEO4J_POD}" -- \
    cypher-shell -u neo4j -p underpassai \
    "MATCH (n:ProjectionNode) WHERE n.node_id IN [${cypher_ids}] DETACH DELETE n RETURN count(*) AS deleted" >/dev/null
}

verify_cleanup() {
  local valkey_pod
  local redis_exists
  local remaining
  local cypher_ids

  valkey_pod="$(find_valkey_pod)"
  redis_exists="$(kubectl exec -n "${KERNEL_NAMESPACE}" "${valkey_pod}" -- \
    redis-cli EXISTS "rehydration:snapshot:${ROOT_NODE_ID}:developer" "rehydration:snapshot:${ROOT_NODE_ID}:reviewer")"
  if [[ "${redis_exists}" != "0" ]]; then
    echo "starship snapshot keys still exist after cleanup" >&2
    exit 1
  fi

  cypher_ids="'${STARSHIP_NODE_IDS[0]}'"
  for node_id in "${STARSHIP_NODE_IDS[@]:1}"; do
    cypher_ids="${cypher_ids}, '${node_id}'"
  done

  remaining="$(
    kubectl exec -n "${GRAPH_NAMESPACE}" "${NEO4J_POD}" -- \
      cypher-shell -u neo4j -p underpassai \
      "MATCH (n:ProjectionNode) WHERE n.node_id IN [${cypher_ids}] RETURN count(n) AS remaining" \
      | tail -n 1 | tr -d '\r'
  )"
  if [[ "${remaining}" != "0" ]]; then
    echo "starship graph nodes still exist after cleanup: ${remaining}" >&2
    exit 1
  fi
}

seed_and_verify() {
  start_port_forwards
  CLUSTER_STARSHIP_GRPC_ENDPOINT="http://127.0.0.1:${LOCAL_GRPC_PORT}" \
  CLUSTER_STARSHIP_NATS_URL="nats://127.0.0.1:${LOCAL_NATS_PORT}" \
  CLUSTER_STARSHIP_SUBJECT_PREFIX="${SUBJECT_PREFIX}" \
    cargo run --offline -p rehydration-transport-grpc --bin starship_cluster_journey
}

case "${ACTION}" in
  run)
    cleanup_starship_data
    seed_and_verify
    if bool_is_true "${AUTO_CLEANUP}"; then
      cleanup_starship_data
      verify_cleanup
    fi
    ;;
  seed-verify)
    seed_and_verify
    ;;
  cleanup)
    cleanup_starship_data
    verify_cleanup
    ;;
  *)
    echo "usage: $0 [run|seed-verify|cleanup]" >&2
    exit 1
    ;;
esac
