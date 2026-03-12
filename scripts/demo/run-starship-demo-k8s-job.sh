#!/usr/bin/env bash

set -euo pipefail

NAMESPACE="${NAMESPACE:-underpass-runtime}"
DEPLOYMENT_NAME="${DEPLOYMENT_NAME:-rehydration-kernel}"
JOB_NAME="${JOB_NAME:-starship-demo-$(date +%s)}"
WAIT_TIMEOUT="${WAIT_TIMEOUT:-15m}"
LLM_PROVIDER="${LLM_PROVIDER:-vllm}"
KERNEL_GRPC_ENDPOINT="${KERNEL_GRPC_ENDPOINT:-http://rehydration-kernel:50054}"
NATS_URL="${NATS_URL:-nats://nats:4222}"
STARSHIP_RUNTIME_MODE="${STARSHIP_RUNTIME_MODE:-filesystem}"
STARSHIP_WORKSPACE_DIR="${STARSHIP_WORKSPACE_DIR:-/workspace-demo}"
STARSHIP_RESET_WORKSPACE="${STARSHIP_RESET_WORKSPACE:-true}"
AGENTIC_DEBUG="${AGENTIC_DEBUG:-${AGENTIC_E2E_DEBUG:-true}}"

STARSHIP_IMAGE="${STARSHIP_IMAGE:-$(
  kubectl get deployment "${DEPLOYMENT_NAME}" \
    -n "${NAMESPACE}" \
    -o jsonpath='{.spec.template.spec.containers[0].image}'
)}"

IMAGE_PULL_SECRET="${IMAGE_PULL_SECRET:-}"
if [[ -z "${IMAGE_PULL_SECRET}" ]]; then
  IMAGE_PULL_SECRET="$(
    kubectl get deployment "${DEPLOYMENT_NAME}" \
      -n "${NAMESPACE}" \
      -o jsonpath='{.spec.template.spec.imagePullSecrets[0].name}' 2>/dev/null || true
  )"
fi

case "${LLM_PROVIDER}" in
  vllm)
    : "${VLLM_MODEL:?VLLM_MODEL is required when LLM_PROVIDER=vllm}"
    VLLM_BASE_URL="${VLLM_BASE_URL:-http://vllm-server:8000}"
    ;;
  openai)
    : "${OPENAI_API_KEY:?OPENAI_API_KEY is required when LLM_PROVIDER=openai}"
    : "${OPENAI_MODEL:?OPENAI_MODEL is required when LLM_PROVIDER=openai}"
    OPENAI_BASE_URL="${OPENAI_BASE_URL:-https://api.openai.com}"
    ;;
  anthropic|claude)
    : "${ANTHROPIC_API_KEY:?ANTHROPIC_API_KEY is required when LLM_PROVIDER=anthropic}"
    : "${ANTHROPIC_MODEL:?ANTHROPIC_MODEL is required when LLM_PROVIDER=anthropic}"
    ANTHROPIC_BASE_URL="${ANTHROPIC_BASE_URL:-https://api.anthropic.com}"
    LLM_PROVIDER="anthropic"
    ;;
  openai_compat|openai-compatible)
    : "${OPENAI_MODEL:?OPENAI_MODEL is required when LLM_PROVIDER=openai_compat}"
    : "${OPENAI_COMPAT_BASE_URL:?OPENAI_COMPAT_BASE_URL is required when LLM_PROVIDER=openai_compat}"
    LLM_PROVIDER="openai_compat"
    ;;
  *)
    echo "unsupported LLM_PROVIDER: ${LLM_PROVIDER}" >&2
    exit 1
    ;;
esac

image_pull_secrets_block=""
if [[ -n "${IMAGE_PULL_SECRET}" ]]; then
  image_pull_secrets_block="$(cat <<EOF
      imagePullSecrets:
        - name: ${IMAGE_PULL_SECRET}
EOF
)"
fi

provider_env_block=""
case "${LLM_PROVIDER}" in
  vllm)
    provider_env_block="$(cat <<EOF
            - name: VLLM_BASE_URL
              value: ${VLLM_BASE_URL}
            - name: VLLM_MODEL
              value: ${VLLM_MODEL}
EOF
)"
    if [[ -n "${VLLM_API_KEY:-}" ]]; then
      provider_env_block+=$'\n'"$(cat <<EOF
            - name: VLLM_API_KEY
              value: ${VLLM_API_KEY}
EOF
)"
    fi
    ;;
  openai)
    provider_env_block="$(cat <<EOF
            - name: OPENAI_BASE_URL
              value: ${OPENAI_BASE_URL}
            - name: OPENAI_MODEL
              value: ${OPENAI_MODEL}
            - name: OPENAI_API_KEY
              value: ${OPENAI_API_KEY}
EOF
)"
    ;;
  anthropic)
    provider_env_block="$(cat <<EOF
            - name: ANTHROPIC_BASE_URL
              value: ${ANTHROPIC_BASE_URL}
            - name: ANTHROPIC_MODEL
              value: ${ANTHROPIC_MODEL}
            - name: ANTHROPIC_API_KEY
              value: ${ANTHROPIC_API_KEY}
EOF
)"
    ;;
  openai_compat)
    provider_env_block="$(cat <<EOF
            - name: OPENAI_COMPAT_BASE_URL
              value: ${OPENAI_COMPAT_BASE_URL}
            - name: OPENAI_MODEL
              value: ${OPENAI_MODEL}
EOF
)"
    if [[ -n "${OPENAI_API_KEY:-}" ]]; then
      provider_env_block+=$'\n'"$(cat <<EOF
            - name: OPENAI_API_KEY
              value: ${OPENAI_API_KEY}
EOF
)"
    fi
    if [[ -n "${OPENAI_COMPAT_API_KEY:-}" ]]; then
      provider_env_block+=$'\n'"$(cat <<EOF
            - name: OPENAI_COMPAT_API_KEY
              value: ${OPENAI_COMPAT_API_KEY}
EOF
)"
    fi
    ;;
esac

manifest_path="$(mktemp "${TMPDIR:-/tmp}/starship-demo-job.XXXXXX.yaml")"
trap 'rm -f "${manifest_path}"' EXIT

cat > "${manifest_path}" <<EOF
apiVersion: batch/v1
kind: Job
metadata:
  name: ${JOB_NAME}
  namespace: ${NAMESPACE}
spec:
  backoffLimit: 0
  ttlSecondsAfterFinished: 3600
  template:
    spec:
${image_pull_secrets_block}
      securityContext:
        runAsNonRoot: true
        runAsUser: 999
        runAsGroup: 999
        fsGroup: 999
        seccompProfile:
          type: RuntimeDefault
      restartPolicy: Never
      containers:
        - name: starship-demo
          image: ${STARSHIP_IMAGE}
          imagePullPolicy: Always
          command:
            - /usr/local/bin/starship-demo-runner
          env:
            - name: KERNEL_GRPC_ENDPOINT
              value: ${KERNEL_GRPC_ENDPOINT}
            - name: NATS_URL
              value: ${NATS_URL}
            - name: STARSHIP_RUNTIME_MODE
              value: ${STARSHIP_RUNTIME_MODE}
            - name: STARSHIP_WORKSPACE_DIR
              value: ${STARSHIP_WORKSPACE_DIR}
            - name: STARSHIP_RESET_WORKSPACE
              value: "${STARSHIP_RESET_WORKSPACE}"
            - name: LLM_PROVIDER
              value: ${LLM_PROVIDER}
            - name: AGENTIC_DEBUG
              value: "${AGENTIC_DEBUG}"
${provider_env_block}
          volumeMounts:
            - name: workspace
              mountPath: ${STARSHIP_WORKSPACE_DIR}
      volumes:
        - name: workspace
          emptyDir: {}
EOF

kubectl apply -f "${manifest_path}"
if ! kubectl wait --for=condition=complete "job/${JOB_NAME}" -n "${NAMESPACE}" --timeout="${WAIT_TIMEOUT}"; then
  pod_name="$(
    kubectl get pods -n "${NAMESPACE}" -l "job-name=${JOB_NAME}" -o jsonpath='{.items[0].metadata.name}'
  )"
  kubectl logs -n "${NAMESPACE}" "${pod_name}" || true
  kubectl describe pod -n "${NAMESPACE}" "${pod_name}" || true
  exit 1
fi

pod_name="$(
  kubectl get pods -n "${NAMESPACE}" -l "job-name=${JOB_NAME}" -o jsonpath='{.items[0].metadata.name}'
)"
kubectl logs -n "${NAMESPACE}" "${pod_name}"
