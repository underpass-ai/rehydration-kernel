#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

LLM_PROVIDER="${LLM_PROVIDER:-vllm}"

case "${LLM_PROVIDER}" in
  vllm)
    [[ -n "${VLLM_BASE_URL:-}" && -n "${VLLM_MODEL:-}" ]] || {
      echo "LLM_PROVIDER=vllm requires VLLM_BASE_URL and VLLM_MODEL" >&2
      exit 1
    }
    ;;
  openai)
    [[ -n "${OPENAI_API_KEY:-}" && -n "${OPENAI_MODEL:-}" ]] || {
      echo "LLM_PROVIDER=openai requires OPENAI_API_KEY and OPENAI_MODEL" >&2
      exit 1
    }
    ;;
  anthropic|claude)
    [[ -n "${ANTHROPIC_API_KEY:-}" && -n "${ANTHROPIC_MODEL:-}" ]] || {
      echo "LLM_PROVIDER=anthropic requires ANTHROPIC_API_KEY and ANTHROPIC_MODEL" >&2
      exit 1
    }
    ;;
  openai_compat|openai-compatible)
    [[ -n "${OPENAI_COMPAT_BASE_URL:-}" && -n "${OPENAI_MODEL:-}" ]] || {
      echo "LLM_PROVIDER=openai_compat requires OPENAI_COMPAT_BASE_URL and OPENAI_MODEL" >&2
      exit 1
    }
    ;;
  *)
    echo "unsupported LLM_PROVIDER: ${LLM_PROVIDER}" >&2
    exit 1
    ;;
esac

if [[ -z "${OPENAI_BASE_URL:-}" && "${LLM_PROVIDER}" == "openai" ]]; then
  export OPENAI_BASE_URL="https://api.openai.com"
fi

if [[ -z "${OPENAI_COMPAT_BASE_URL:-}" && "${LLM_PROVIDER}" == "openai-compatible" ]]; then
  export OPENAI_COMPAT_BASE_URL="${OPENAI_BASE_URL:-}"
fi

if [[ -z "${OPENAI_COMPAT_BASE_URL:-}" && "${LLM_PROVIDER}" == "openai_compat" ]]; then
  export OPENAI_COMPAT_BASE_URL="${OPENAI_BASE_URL:-}"
fi

if [[ "${LLM_PROVIDER}" == "claude" ]]; then
  export LLM_PROVIDER="anthropic"
fi

if [[ "${LLM_PROVIDER}" == "openai-compatible" ]]; then
  export LLM_PROVIDER="openai_compat"
fi

if [[ -z "${LLM_PROVIDER:-}" ]]; then
  echo "missing LLM_PROVIDER" >&2
  exit 1
fi

cd "${ROOT_DIR}"
. "${ROOT_DIR}/scripts/ci/testcontainers-runtime.sh"

cargo test \
  -p rehydration-transport-grpc \
  --features container-tests \
  --test starship_real_llm_demo \
  --locked \
  -- \
  --ignored \
  --nocapture \
  --test-threads=1
