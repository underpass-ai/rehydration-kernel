#!/usr/bin/env bash

set -euo pipefail

ACTION="${1:-run}"
KERNEL_NAMESPACE="${KERNEL_NAMESPACE:-underpass-runtime}"
GRAPH_NAMESPACE="${GRAPH_NAMESPACE:-swe-ai-fleet}"
KERNEL_SERVICE="${KERNEL_SERVICE:-rehydration-kernel}"
NATS_SERVICE="${NATS_SERVICE:-nats}"
KERNEL_DEPLOYMENT="${KERNEL_DEPLOYMENT:-rehydration-kernel}"
NEO4J_POD="${NEO4J_POD:-neo4j-0}"
SUBJECT_PREFIX="${SUBJECT_PREFIX:-rehydration}"
AUTO_CLEANUP="${AUTO_CLEANUP:-true}"
JOB_TIMEOUT="${JOB_TIMEOUT:-5m}"
JOB_PREFIX="${JOB_PREFIX:-starship-cluster-journey}"
GRPC_TLS_MODE="${GRPC_TLS_MODE:-disabled}"
GRPC_TLS_SECRET_NAME="${GRPC_TLS_SECRET_NAME:-}"
GRPC_TLS_CA_KEY="${GRPC_TLS_CA_KEY:-ca.crt}"
GRPC_TLS_CERT_KEY="${GRPC_TLS_CERT_KEY:-client.crt}"
GRPC_TLS_KEY_KEY="${GRPC_TLS_KEY_KEY:-client.key}"
NATS_TLS_MODE="${NATS_TLS_MODE:-disabled}"
NATS_TLS_SECRET_NAME="${NATS_TLS_SECRET_NAME:-}"
NATS_TLS_FIRST="${NATS_TLS_FIRST:-false}"
NATS_TLS_CA_KEY="${NATS_TLS_CA_KEY:-ca.crt}"
NATS_TLS_CERT_KEY="${NATS_TLS_CERT_KEY:-tls.crt}"
NATS_TLS_KEY_KEY="${NATS_TLS_KEY_KEY:-tls.key}"
JOB_GRPC_TLS_MOUNT_PATH="${JOB_GRPC_TLS_MOUNT_PATH:-/var/run/starship-cluster-journey/grpc-tls}"
JOB_NATS_TLS_MOUNT_PATH="${JOB_NATS_TLS_MOUNT_PATH:-/var/run/starship-cluster-journey/nats-tls}"
JOB_NAME=""

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
  "workstream:containment-control-loop"
  "checklist:align-plasma-baffles"
  "artifact:plasma-baffle-telemetry"
)

bool_is_true() {
  if [[ "${1}" == "true" || "${1}" == "1" || "${1}" == "yes" ]]; then
    return 0
  fi
  return 1
}

find_valkey_pod() {
  kubectl get pods -n "${KERNEL_NAMESPACE}" -o name \
    | grep '^pod/valkey-' \
    | head -n 1 \
    | cut -d/ -f2
  return 0
}

validate_tls_inputs() {
  case "${GRPC_TLS_MODE}" in
    disabled|server|mutual) ;;
    *)
      echo "GRPC_TLS_MODE must be disabled, server, or mutual" >&2
      exit 1
      ;;
  esac

  case "${NATS_TLS_MODE}" in
    disabled|server|mutual) ;;
    *)
      echo "NATS_TLS_MODE must be disabled, server, or mutual" >&2
      exit 1
      ;;
  esac

  if [[ "${GRPC_TLS_MODE}" != "disabled" && -z "${GRPC_TLS_SECRET_NAME}" ]]; then
    echo "GRPC_TLS_SECRET_NAME is required when GRPC_TLS_MODE is server or mutual" >&2
    exit 1
  fi

  if [[ "${GRPC_TLS_MODE}" == "mutual" ]]; then
    if [[ -z "${GRPC_TLS_CERT_KEY}" || -z "${GRPC_TLS_KEY_KEY}" ]]; then
      echo "GRPC mutual TLS requires GRPC_TLS_CERT_KEY and GRPC_TLS_KEY_KEY" >&2
      exit 1
    fi
  fi

  if [[ "${NATS_TLS_MODE}" != "disabled" && -z "${NATS_TLS_SECRET_NAME}" ]]; then
    echo "NATS_TLS_SECRET_NAME is required when NATS_TLS_MODE is server or mutual" >&2
    exit 1
  fi

  if [[ "${NATS_TLS_MODE}" == "mutual" ]]; then
    if [[ -z "${NATS_TLS_CERT_KEY}" || -z "${NATS_TLS_KEY_KEY}" ]]; then
      echo "NATS mutual TLS requires NATS_TLS_CERT_KEY and NATS_TLS_KEY_KEY" >&2
      exit 1
    fi
  fi

  if bool_is_true "${NATS_TLS_FIRST}" && [[ "${NATS_TLS_MODE}" == "disabled" ]]; then
    echo "NATS_TLS_FIRST requires NATS_TLS_MODE=server or mutual" >&2
    exit 1
  fi
}

kernel_image() {
  kubectl get deploy -n "${KERNEL_NAMESPACE}" "${KERNEL_DEPLOYMENT}" \
    -o=jsonpath='{.spec.template.spec.containers[0].image}'
  return 0
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
  return 0
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
  return 0
}

delete_job_if_present() {
  if [[ -n "${JOB_NAME}" ]]; then
    kubectl delete job -n "${KERNEL_NAMESPACE}" "${JOB_NAME}" --ignore-not-found >/dev/null
  fi
  return 0
}

cleanup_job_on_exit() {
  if bool_is_true "${AUTO_CLEANUP}"; then
    delete_job_if_present
  fi
  return 0
}

job_manifest() {
  local image="$1"
  local grpc_host="${KERNEL_SERVICE}.${KERNEL_NAMESPACE}.svc.cluster.local"
  local grpc_scheme="http"
  local grpc_tls_env=""
  local grpc_tls_mount=""
  local grpc_tls_volume=""
  local nats_tls_env=""
  local nats_tls_mount=""
  local nats_tls_volume=""
  local volume_mounts_block=""
  local volumes_block=""

  if [[ "${GRPC_TLS_MODE}" != "disabled" ]]; then
    grpc_scheme="https"
    grpc_tls_env="$(cat <<EOF
            - name: CLUSTER_STARSHIP_GRPC_TLS_MODE
              value: ${GRPC_TLS_MODE}
            - name: CLUSTER_STARSHIP_GRPC_TLS_DOMAIN_NAME
              value: ${grpc_host}
            - name: CLUSTER_STARSHIP_GRPC_TLS_CA_PATH
              value: ${JOB_GRPC_TLS_MOUNT_PATH}/${GRPC_TLS_CA_KEY}
EOF
)"
    if [[ "${GRPC_TLS_MODE}" == "mutual" ]]; then
      grpc_tls_env="${grpc_tls_env}"$'\n'"$(cat <<EOF
            - name: CLUSTER_STARSHIP_GRPC_TLS_CERT_PATH
              value: ${JOB_GRPC_TLS_MOUNT_PATH}/${GRPC_TLS_CERT_KEY}
            - name: CLUSTER_STARSHIP_GRPC_TLS_KEY_PATH
              value: ${JOB_GRPC_TLS_MOUNT_PATH}/${GRPC_TLS_KEY_KEY}
EOF
)"
    fi
    grpc_tls_mount="$(cat <<EOF
            - name: grpc-tls
              mountPath: ${JOB_GRPC_TLS_MOUNT_PATH}
              readOnly: true
EOF
)"
    grpc_tls_volume="$(cat <<EOF
        - name: grpc-tls
          secret:
            secretName: ${GRPC_TLS_SECRET_NAME}
EOF
)"
  fi

  if [[ "${NATS_TLS_MODE}" != "disabled" ]]; then
    nats_tls_env="$(cat <<EOF
            - name: CLUSTER_STARSHIP_NATS_TLS_MODE
              value: ${NATS_TLS_MODE}
            - name: CLUSTER_STARSHIP_NATS_TLS_CA_PATH
              value: ${JOB_NATS_TLS_MOUNT_PATH}/${NATS_TLS_CA_KEY}
EOF
)"
    if bool_is_true "${NATS_TLS_FIRST}"; then
      nats_tls_env="${nats_tls_env}"$'\n'"$(cat <<EOF
            - name: CLUSTER_STARSHIP_NATS_TLS_FIRST
              value: "true"
EOF
)"
    fi
    if [[ "${NATS_TLS_MODE}" == "mutual" ]]; then
      nats_tls_env="${nats_tls_env}"$'\n'"$(cat <<EOF
            - name: CLUSTER_STARSHIP_NATS_TLS_CERT_PATH
              value: ${JOB_NATS_TLS_MOUNT_PATH}/${NATS_TLS_CERT_KEY}
            - name: CLUSTER_STARSHIP_NATS_TLS_KEY_PATH
              value: ${JOB_NATS_TLS_MOUNT_PATH}/${NATS_TLS_KEY_KEY}
EOF
)"
    fi
    nats_tls_mount="$(cat <<EOF
            - name: nats-tls
              mountPath: ${JOB_NATS_TLS_MOUNT_PATH}
              readOnly: true
EOF
)"
    nats_tls_volume="$(cat <<EOF
        - name: nats-tls
          secret:
            secretName: ${NATS_TLS_SECRET_NAME}
EOF
)"
  fi

  if [[ -n "${grpc_tls_mount}${nats_tls_mount}" ]]; then
    volume_mounts_block="$(cat <<EOF
          volumeMounts:
${grpc_tls_mount:+${grpc_tls_mount}}
${nats_tls_mount:+${nats_tls_mount}}
EOF
)"
  fi

  if [[ -n "${grpc_tls_volume}${nats_tls_volume}" ]]; then
    volumes_block="$(cat <<EOF
      volumes:
${grpc_tls_volume:+${grpc_tls_volume}}
${nats_tls_volume:+${nats_tls_volume}}
EOF
)"
  fi

  cat <<EOF
apiVersion: batch/v1
kind: Job
metadata:
  name: ${JOB_NAME}
  namespace: ${KERNEL_NAMESPACE}
spec:
  backoffLimit: 0
  ttlSecondsAfterFinished: 300
  template:
    spec:
      restartPolicy: Never
      containers:
        - name: starship-cluster-journey
          image: ${image}
          imagePullPolicy: Always
          command:
            - /usr/local/bin/starship-cluster-journey
          env:
            - name: CLUSTER_STARSHIP_GRPC_ENDPOINT
              value: ${grpc_scheme}://${grpc_host}:50054
            - name: CLUSTER_STARSHIP_NATS_URL
              value: nats://${NATS_SERVICE}.${KERNEL_NAMESPACE}.svc.cluster.local:4222
            - name: CLUSTER_STARSHIP_SUBJECT_PREFIX
              value: ${SUBJECT_PREFIX}
${grpc_tls_env:+${grpc_tls_env}}
${nats_tls_env:+${nats_tls_env}}
${volume_mounts_block:+${volume_mounts_block}}
${volumes_block:+${volumes_block}}
EOF
}

run_job() {
  local image

  image="$(kernel_image)"
  if [[ -z "${image}" ]]; then
    echo "could not resolve current kernel image from deployment ${KERNEL_DEPLOYMENT}" >&2
    exit 1
  fi

  JOB_NAME="${JOB_PREFIX}-$(date +%s)"
  job_manifest "${image}" | kubectl apply -f - >/dev/null

  set +e
  kubectl wait -n "${KERNEL_NAMESPACE}" --for=condition=complete "job/${JOB_NAME}" --timeout="${JOB_TIMEOUT}"
  local status=$?
  set -e

  kubectl logs -n "${KERNEL_NAMESPACE}" "job/${JOB_NAME}"

  if [[ ${status} -ne 0 ]]; then
    kubectl describe job -n "${KERNEL_NAMESPACE}" "${JOB_NAME}" >&2 || true
    kubectl get pods -n "${KERNEL_NAMESPACE}" -l "job-name=${JOB_NAME}" >&2 || true
    exit 1
  fi
  return 0
}

trap cleanup_job_on_exit EXIT
validate_tls_inputs

case "${ACTION}" in
  run)
    cleanup_starship_data
    run_job
    if bool_is_true "${AUTO_CLEANUP}"; then
      cleanup_starship_data
      verify_cleanup
    fi
    ;;
  seed-verify)
    run_job
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
