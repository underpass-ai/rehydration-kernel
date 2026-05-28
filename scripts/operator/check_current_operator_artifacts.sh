#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ARTIFACT_ROOT="${OPERATOR_ARTIFACT_ROOT:-${ROOT_DIR}/../rehydration-kernel-artifacts/operator/2026-05-17-kmp-cursor-contract-refresh}"

cd "${ROOT_DIR}"

check_file() {
  local file="$1"

  if [[ ! -f "${file}" ]]; then
    printf 'missing Operator artifact: %s\n' "${file}" >&2
    return 1
  fi
}

check_sha256() {
  local file="$1"
  local expected="$2"
  local actual

  check_file "${file}"
  actual="$(sha256sum "${file}" | awk '{print $1}')"
  if [[ "${actual}" != "${expected}" ]]; then
    printf 'Operator artifact hash mismatch: %s\nexpected: %s\nactual:   %s\n' \
      "${file}" "${expected}" "${actual}" >&2
    return 1
  fi
}

coverage() {
  local profile="$1"
  local file="$2"

  check_file "${file}"
  cargo run -p underpass-operator-evaluation-cli --bin underpass_operator_contract_coverage -- \
    --profile "${profile}" \
    --trajectories "${file}" \
    --fail-under 100 >/dev/null
}

train_validate_only() {
  local dataset_root="$1"
  local output_dir="$2"

  python scripts/operator/train_operator_sft_lora.py \
    --train-jsonl "${dataset_root}/openai_train.jsonl" \
    --eval-jsonl "${dataset_root}/openai_eval.jsonl" \
    --output-dir "${output_dir}" \
    --validate-only >/dev/null
}

predict_validate_only() {
  local dataset_root="$1"
  local output_dir="$2"
  shift 2

  python scripts/operator/predict_operator_sft.py \
    --dataset-jsonl "${dataset_root}/eval.jsonl" \
    --model-id Qwen/Qwen2.5-0.5B-Instruct \
    --output "${output_dir}" \
    "$@" \
    --validate-only >/dev/null
}

read_sft="${ARTIFACT_ROOT}/kernel-operator-sft-read-api-mcp-v1-kmp-cursor-20260517"
wpr_sft="${ARTIFACT_ROOT}/kernel-operator-sft-writer-pre-read-v4-kmp-cursor-v2-20260517"
wexec_sft="${ARTIFACT_ROOT}/kernel-operator-sft-writer-exec-v1-prepared-exec-source-kind-agent-20260517"
worch_sft="${ARTIFACT_ROOT}/kernel-operator-sft-writer-orchestration-v2-kmp-cursor-20260517"
run_root="${OPERATOR_RUN_ROOT:-/tmp/kernel-operator-current-artifact-preflight}"

check_sha256 "${read_sft}/summary.json" "0e2974fcedb446f6f0e8428de1a43ff0f744fc68a26a9874705f30d146370d28"
check_sha256 "${read_sft}/train.jsonl" "a663da76113fcd4a275a1f3af91ec20e65a9248651c3f56aa8705ad400ccaa71"
check_sha256 "${read_sft}/eval.jsonl" "db71a840d96a5a142c577d6bb740c3bb09f5fdf16fb82f1119194f7daab54cc8"
check_sha256 "${read_sft}/openai_train.jsonl" "d9a0f5fd5050b88ca12968e04ad469886c90db3125d31332572243d7440fa0f6"
check_sha256 "${read_sft}/openai_eval.jsonl" "5e09edb27c0e7ba9c0c3776c676c6fbf04cc8620bca1ab4bc38a1f1ed577cd72"

check_sha256 "${wpr_sft}/summary.json" "d57db2797eab07409cd83ab5974b55990c05304cc39320a4e348b0071099f7ce"
check_sha256 "${wpr_sft}/train.jsonl" "87d7e41f1869af14332f22c42da814283d7a5b21026fede92520ffee75230491"
check_sha256 "${wpr_sft}/eval.jsonl" "278d7d5bacd1f85f6f401b227d83ffb1fe3fb35fe08ab8216f0401e003da858d"
check_sha256 "${wpr_sft}/openai_train.jsonl" "1ff100e1bb00ebc10ae0ccbc0e19a27d50d7f93375f1045fd8ac3f2148775bc6"
check_sha256 "${wpr_sft}/openai_eval.jsonl" "395fdc1fdc87f3b70f5799c265a1d2baa97fe0f55673d2da09ebaaa1f3e73040"

check_sha256 "${wexec_sft}/summary.json" "6df2a058031d3d2ec1fb0f4ec88b5dfb25cc9d74265c452d7c1ac871281af498"
check_sha256 "${wexec_sft}/train.jsonl" "43cdd49f6f7b8b20ae79ea970a208678a2d8fefd02ea6ee96c20b934356abc76"
check_sha256 "${wexec_sft}/eval.jsonl" "162331bbcd256d9cfd837545c7d7bcdcd83cd75e7972e2d7fdce5c56f05c1180"
check_sha256 "${wexec_sft}/openai_train.jsonl" "5f80541e90d1fb3235b76385c913a2123e6dd6988b06e5cee0fa1084a2ea0dbe"
check_sha256 "${wexec_sft}/openai_eval.jsonl" "d2ee1c5cae3d5754b194aeaa1c984135452f824a56383f87fe89fb8c43f4f875"

check_sha256 "${worch_sft}/summary.json" "4fe0f17c25a98a7157c2f28547a051baa41e0ae586fc68ae673d0ae83b8a3227"
check_sha256 "${worch_sft}/train.jsonl" "3cc50ea0f076949e7b004b61df92171a9463ad32813f2c04d7a542dcf05e8d4f"
check_sha256 "${worch_sft}/eval.jsonl" "61b4d8b3abdc1c64aba7b7d64804a1ee6dd1cefd8d0a3c96db8f25d8ede37979"
check_sha256 "${worch_sft}/openai_train.jsonl" "5069d2bc9bb8f1a325faf92d26500629f7a69240f0e6bb1dd81ced075f5bf4c6"
check_sha256 "${worch_sft}/openai_eval.jsonl" "e4b62da25bdbc6dda3eee90b9cf87666fcada802ce87dd0acec92e22cb58b45b"
check_sha256 "${worch_sft}/oracle-policy-eval.json" "57126a63128acb5822564a1ac8eae183ae9e3558a75e26e371e3ce7064d1e247"

train_validate_only "${read_sft}" "${run_root}/read-api-mcp-v1-kmp-cursor-train-validate-only"
train_validate_only "${wpr_sft}" "${run_root}/writer-pre-read-v4-kmp-cursor-v2-train-validate-only"
train_validate_only "${wexec_sft}" "${run_root}/writer-exec-prepared-source-kind-agent-train-validate-only"
train_validate_only "${worch_sft}" "${run_root}/writer-orchestration-v2-kmp-cursor-train-validate-only"

predict_validate_only "${read_sft}" "${run_root}/read-api-mcp-v1-kmp-cursor-predict-validate-only"
predict_validate_only "${wpr_sft}" "${run_root}/writer-pre-read-v4-kmp-cursor-v2-predict-validate-only"
predict_validate_only "${wexec_sft}" "${run_root}/writer-exec-prepared-source-kind-agent-predict-validate-only" --resolve-prepared-payloads
predict_validate_only "${worch_sft}" "${run_root}/writer-orchestration-v2-kmp-cursor-predict-validate-only" --resolve-prepared-payloads

for split in train eval; do
  coverage read "${read_sft}/${split}.jsonl"
  coverage writer-pre-read "${wpr_sft}/${split}.jsonl"
  coverage write "${wexec_sft}/${split}.jsonl"
  coverage writer-pre-read "${worch_sft}/${split}.jsonl"
  coverage write "${worch_sft}/${split}.jsonl"
done

bash scripts/ci/check-operator-k8s-jobs.sh >/dev/null

printf 'current Operator artifacts: ok\n'
