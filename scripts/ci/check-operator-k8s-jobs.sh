#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "${ROOT_DIR}"

readonly QUARANTINE_ANNOTATION="underpass.ai/quarantine-reason"
readonly OPERATOR_ARTIFACTS_HOST_PATH="/home/tirso/ai/developents/rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh"
readonly OPERATOR_RUNS_HOST_PATH="/home/tirso/ai/developents/rehydration-kernel-artifacts/operator/runs"

readonly -a CURRENT_OPERATOR_JOBS=(
  "k8s/kernel-operator-qwen05-lora-read-api-mcp-v1-kmp-cursor-4gpu-20260517-job.yaml"
  "k8s/kernel-operator-qwen05-predict-read-api-mcp-v1-kmp-cursor-20260517-job.yaml"
  "k8s/kernel-operator-qwen05-lora-writer-pre-read-v4-kmp-cursor-v2-4gpu-20260517-job.yaml"
  "k8s/kernel-operator-qwen05-predict-writer-pre-read-v4-kmp-cursor-v2-20260517-job.yaml"
  "k8s/kernel-operator-qwen05-lora-writer-exec-prepared-source-kind-agent-4gpu-20260517-job.yaml"
  "k8s/kernel-operator-qwen05-predict-writer-exec-prepared-source-kind-agent-20260517-job.yaml"
  "k8s/kernel-operator-qwen05-lora-writer-orchestration-v2-kmp-cursor-4gpu-20260517-job.yaml"
  "k8s/kernel-operator-qwen05-predict-writer-orchestration-v2-kmp-cursor-20260517-job.yaml"
)

failures=0

fail() {
  printf 'operator k8s job policy violation: %s\n' "$1" >&2
  failures=$((failures + 1))
}

require_contains() {
  local file="$1"
  local expected="$2"
  local description="$3"

  if ! grep -Fq -- "${expected}" "${file}"; then
    fail "${file} is missing ${description}: ${expected}"
  fi
}

require_volume_mount() {
  local file="$1"
  local name="$2"
  local mount_path="$3"
  local read_only="$4"

  if ! awk -v name="${name}" -v mount_path="${mount_path}" -v read_only="${read_only}" '
    $0 == "          volumeMounts:" {
      in_mounts = 1
      next
    }
    in_mounts && $0 == "      volumes:" {
      in_mounts = 0
      in_target = 0
    }
    in_mounts && $0 == "            - name: " name {
      found = 1
      in_target = 1
      next
    }
    in_target && $0 ~ /^            - name: / {
      in_target = 0
    }
    in_target && $0 == "              mountPath: " mount_path {
      found_mount = 1
    }
    in_target && $0 == "              readOnly: true" {
      found_read_only = 1
    }
    END {
      if (!found || !found_mount) {
        exit 1
      }
      if (read_only == "true" && !found_read_only) {
        exit 1
      }
    }
  ' "${file}"; then
    fail "${file} does not mount ${name} at ${mount_path} with the required options"
  fi
}

require_host_path_volume() {
  local file="$1"
  local name="$2"
  local host_path="$3"
  local host_type="$4"

  if ! awk -v name="${name}" -v host_path="${host_path}" -v host_type="${host_type}" '
    $0 == "      volumes:" {
      in_volumes = 1
      next
    }
    in_volumes && $0 == "        - name: " name {
      found = 1
      in_target = 1
      next
    }
    in_target && $0 ~ /^        - name: / {
      in_target = 0
    }
    in_target && $0 == "            path: " host_path {
      found_path = 1
    }
    in_target && $0 == "            type: " host_type {
      found_type = 1
    }
    END {
      if (!found || !found_path || !found_type) {
        exit 1
      }
    }
  ' "${file}"; then
    fail "${file} does not define ${name} as hostPath ${host_path} (${host_type})"
  fi
}

is_current_job() {
  local candidate="$1"
  local current

  for current in "${CURRENT_OPERATOR_JOBS[@]}"; do
    if [[ "${candidate}" == "${current}" ]]; then
      return 0
    fi
  done

  return 1
}

first_line() {
  local pattern="$1"
  local file="$2"

  awk -v pattern="${pattern}" '
    $0 ~ pattern {
      print NR
      exit
    }
  ' "${file}"
}

require_preflight_before() {
  local file="$1"
  local marker="$2"
  local marker_name="$3"
  local preflight_line
  local marker_line

  preflight_line="$(first_line "--validate-only" "${file}")"
  marker_line="$(first_line "${marker}" "${file}")"

  if [[ -z "${marker_line}" ]]; then
    return 0
  fi

  if [[ -z "${preflight_line}" ]]; then
    fail "${file} does not run validate-only before ${marker_name}"
    return 0
  fi

  if (( preflight_line >= marker_line )); then
    fail "${file} runs validate-only after ${marker_name}"
  fi
}

validate_current_job() {
  local file="$1"

  if grep -Eq '^[[:space:]]*suspend:[[:space:]]*true[[:space:]]*$' "${file}"; then
    fail "${file} is current but is suspended"
  fi

  if grep -q "${QUARANTINE_ANNOTATION}" "${file}"; then
    fail "${file} is current but carries a quarantine annotation"
  fi

  if ! grep -q -- "--validate-only" "${file}"; then
    fail "${file} is current but has no validate-only preflight"
  fi

  if grep -q "/host-tmp/kernel-operator-sft-" "${file}"; then
    fail "${file} reads current training data from /host-tmp instead of /operator-artifacts"
  fi

  require_volume_mount "${file}" "operator-artifacts" "/operator-artifacts" "true"
  require_host_path_volume "${file}" "operator-artifacts" "${OPERATOR_ARTIFACTS_HOST_PATH}" "Directory"

  if grep -q "/host-tmp/kernel-operator-qwen05-" "${file}"; then
    fail "${file} writes current adapters/predictions to /host-tmp instead of /operator-runs"
  fi

  require_volume_mount "${file}" "operator-runs" "/operator-runs" "false"
  require_host_path_volume "${file}" "operator-runs" "${OPERATOR_RUNS_HOST_PATH}" "DirectoryOrCreate"

  require_preflight_before "${file}" "python -m pip install" "dependency install"
  require_preflight_before "${file}" "rm -rf" "output deletion"
  require_preflight_before "${file}" "torchrun" "model training"
  require_preflight_before "${file}" "--adapter" "adapter-backed prediction"

  if [[ "${file}" == *writer-exec-prepared* || "${file}" == *writer-orchestration* ]]; then
    if [[ "${file}" == *predict* ]] && ! grep -q -- "--resolve-prepared-payloads" "${file}"; then
      fail "${file} predicts prepared write actions without --resolve-prepared-payloads"
    fi
  fi

  if grep -q "source_kind=synthetic_conformance" "${file}"; then
    fail "${file} references unsupported source_kind=synthetic_conformance"
  fi

  if grep -q "cursor_not_numeric" "${file}"; then
    fail "${file} references symbolic trace cursor test data"
  fi

  validate_current_job_contract "${file}"
}

validate_lora_job_contract() {
  local file="$1"
  local dataset_root="$2"
  local output_dir="$3"
  local validate_output_dir="$4"

  require_contains "${file}" "--train-jsonl /operator-artifacts/${dataset_root}/openai_train.jsonl" "expected train JSONL"
  require_contains "${file}" "--eval-jsonl /operator-artifacts/${dataset_root}/openai_eval.jsonl" "expected eval JSONL"
  require_contains "${file}" "--output-dir /operator-runs/${validate_output_dir}" "expected validate-only output directory"
  require_contains "${file}" "--output-dir /operator-runs/${output_dir}" "expected training output directory"
}

validate_predict_job_contract() {
  local file="$1"
  local dataset_root="$2"
  local adapter_dir="$3"
  local output_dir="$4"
  local validate_output_dir="$5"

  require_contains "${file}" "--dataset-jsonl /operator-artifacts/${dataset_root}/eval.jsonl" "expected prediction dataset"
  require_contains "${file}" "--output /operator-runs/${validate_output_dir}" "expected validate-only output directory"
  require_contains "${file}" "--adapter /operator-runs/${adapter_dir}" "expected adapter directory"
  require_contains "${file}" "--output /operator-runs/${output_dir}" "expected prediction output directory"
}

validate_current_job_contract() {
  local file="$1"

  case "${file}" in
    k8s/kernel-operator-qwen05-lora-read-api-mcp-v1-kmp-cursor-4gpu-20260517-job.yaml)
      validate_lora_job_contract \
        "${file}" \
        "kernel-operator-sft-read-api-mcp-v1-kmp-cursor-20260517" \
        "kernel-operator-qwen05-lora-read-api-mcp-v1-kmp-cursor-4gpu-20260517" \
        "kernel-operator-qwen05-lora-read-api-mcp-v1-kmp-cursor-validate-only"
      ;;
    k8s/kernel-operator-qwen05-predict-read-api-mcp-v1-kmp-cursor-20260517-job.yaml)
      validate_predict_job_contract \
        "${file}" \
        "kernel-operator-sft-read-api-mcp-v1-kmp-cursor-20260517" \
        "kernel-operator-qwen05-lora-read-api-mcp-v1-kmp-cursor-4gpu-20260517" \
        "kernel-operator-qwen05-predictions-read-api-mcp-v1-kmp-cursor-20260517" \
        "kernel-operator-qwen05-predictions-read-api-mcp-v1-kmp-cursor-validate-only"
      ;;
    k8s/kernel-operator-qwen05-lora-writer-pre-read-v4-kmp-cursor-v2-4gpu-20260517-job.yaml)
      validate_lora_job_contract \
        "${file}" \
        "kernel-operator-sft-writer-pre-read-v4-kmp-cursor-v2-20260517" \
        "kernel-operator-qwen05-lora-wpr-v4-kmp-cursor-v2-4gpu-20260517" \
        "kernel-operator-qwen05-lora-wpr-v4-kmp-cursor-v2-validate-only"
      ;;
    k8s/kernel-operator-qwen05-predict-writer-pre-read-v4-kmp-cursor-v2-20260517-job.yaml)
      validate_predict_job_contract \
        "${file}" \
        "kernel-operator-sft-writer-pre-read-v4-kmp-cursor-v2-20260517" \
        "kernel-operator-qwen05-lora-wpr-v4-kmp-cursor-v2-4gpu-20260517" \
        "kernel-operator-qwen05-predictions-wpr-v4-kmp-cursor-v2-20260517" \
        "kernel-operator-qwen05-predictions-wpr-v4-kmp-cursor-v2-validate-only"
      ;;
    k8s/kernel-operator-qwen05-lora-writer-exec-prepared-source-kind-agent-4gpu-20260517-job.yaml)
      validate_lora_job_contract \
        "${file}" \
        "kernel-operator-sft-writer-exec-v1-prepared-exec-source-kind-agent-20260517" \
        "kernel-operator-qwen05-lora-writer-exec-prepared-source-kind-agent-4gpu-20260517" \
        "kernel-operator-qwen05-lora-writer-exec-prepared-source-kind-agent-validate-only"
      ;;
    k8s/kernel-operator-qwen05-predict-writer-exec-prepared-source-kind-agent-20260517-job.yaml)
      validate_predict_job_contract \
        "${file}" \
        "kernel-operator-sft-writer-exec-v1-prepared-exec-source-kind-agent-20260517" \
        "kernel-operator-qwen05-lora-writer-exec-prepared-source-kind-agent-4gpu-20260517" \
        "kernel-operator-qwen05-predictions-writer-exec-prepared-source-kind-agent-20260517" \
        "kernel-operator-qwen05-predictions-writer-exec-prepared-source-kind-agent-validate-only"
      ;;
    k8s/kernel-operator-qwen05-lora-writer-orchestration-v2-kmp-cursor-4gpu-20260517-job.yaml)
      validate_lora_job_contract \
        "${file}" \
        "kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517" \
        "kernel-operator-qwen05-lora-writer-orchestration-v2-kmp-cursor-4gpu-20260517" \
        "kernel-operator-qwen05-lora-writer-orchestration-v2-kmp-cursor-validate-only"
      ;;
    k8s/kernel-operator-qwen05-predict-writer-orchestration-v2-kmp-cursor-20260517-job.yaml)
      validate_predict_job_contract \
        "${file}" \
        "kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517" \
        "kernel-operator-qwen05-lora-writer-orchestration-v2-kmp-cursor-4gpu-20260517" \
        "kernel-operator-qwen05-predictions-writer-orchestration-v2-kmp-cursor-20260517" \
        "kernel-operator-qwen05-predictions-writer-orchestration-v2-kmp-cursor-validate-only"
      ;;
    *)
      fail "${file} is current but has no explicit job-contract expectation"
      ;;
  esac
}

validate_historical_job() {
  local file="$1"

  if ! grep -Eq '^[[:space:]]*suspend:[[:space:]]*true[[:space:]]*$' "${file}"; then
    fail "${file} is historical but is not suspended"
  fi

  if ! grep -q "${QUARANTINE_ANNOTATION}" "${file}"; then
    fail "${file} is historical but has no quarantine annotation"
  fi
}

main() {
  local current
  local file

  for current in "${CURRENT_OPERATOR_JOBS[@]}"; do
    if [[ ! -f "${current}" ]]; then
      fail "${current} is listed as current but does not exist"
    fi
  done

  while IFS= read -r file; do
    if is_current_job "${file}"; then
      validate_current_job "${file}"
    else
      validate_historical_job "${file}"
    fi
  done < <(find k8s -maxdepth 1 -type f -name 'kernel-operator*.yaml' | sort)

  if (( failures > 0 )); then
    return 1
  fi

  printf 'operator k8s job policy: ok\n'
}

main "$@"
