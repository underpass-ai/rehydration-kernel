#!/usr/bin/env bash
set -euo pipefail
source /app/common.sh

echo "=== async-vllm-blind-context-consumption ==="

require_env LLM_ENDPOINT
require_env LLM_MODEL
require_env LLM_PROVIDER
require_env LLM_SEMANTIC_CLASSIFIER_ENDPOINT
require_env LLM_SEMANTIC_CLASSIFIER_MODEL
require_env LLM_SEMANTIC_CLASSIFIER_PROVIDER

if [[ "${LLM_PROVIDER}" != "openai" ]]; then
  fail "async-vllm-blind-context-consumption requires LLM_PROVIDER=openai"
fi

run_id="${RUN_ID:-e2e-vllm-blind-context-$(date +%s)}"
question="Answer in one concise sentence: which operational change most likely explains the latency spike, and what action reduced user impact while recovery progressed?"
prompt_template_path="/app/api/examples/inference-prompts/kernel-context-consumption.txt"

[[ -f "${prompt_template_path}" ]] || fail "prompt template not found: ${prompt_template_path}"

export PIR_GRAPH_BATCH_INCLUDE_RENDERED_CONTENT=true
: "${PIR_GRAPH_BATCH_REHYDRATION_MODE:=reason_preserving}"
: "${VLLM_REQUEST_KIND:=blind}"
: "${VLLM_REQUEST_USE_SEMANTIC_CLASSIFIER:=true}"
: "${VLLM_REQUEST_NAMESPACE_NODE_IDS:=true}"

request_cmd=()
build_graph_batch_vllm_request_command request_cmd "${run_id}"

roundtrip_cmd=()
build_graph_batch_roundtrip_command roundtrip_cmd "-" "${run_id}"

summary="$("${request_cmd[@]}" | "${roundtrip_cmd[@]}")" || fail "blind context roundtrip pipeline failed"
assert_roundtrip_summary "${summary}"

rendered_context="$(jq -r '.rendered_content // empty' <<<"${summary}")"
[[ -n "${rendered_context}" ]] || fail "roundtrip summary did not include rendered_content"

prompt_template="$(cat "${prompt_template_path}")"
prompt="$(
  jq -rn \
    --arg template "${prompt_template}" \
    --arg question "${question}" \
    --arg rendered_context "${rendered_context}" \
    '$template | gsub("\\{question\\}"; $question) | gsub("\\{rendered_context\\}"; $rendered_context)'
)"

request_body="$(
  jq -n \
    --arg model "${LLM_MODEL}" \
    --arg prompt "${prompt}" \
    --argjson max_tokens 256 \
    '{
      model: $model,
      messages: [{role: "user", content: $prompt}],
      temperature: 0
    } + (if env.LLM_ENABLE_THINKING == "false" or env.LLM_ENABLE_THINKING == "0"
         then {chat_template_kwargs: {enable_thinking: false}}
         else {}
         end)'
)"

curl_args=(
  -sS
  -H "content-type: application/json"
  --data "${request_body}"
)

if [[ -n "${LLM_API_KEY:-}" ]]; then
  curl_args+=(-H "authorization: Bearer ${LLM_API_KEY}")
fi
if [[ -n "${LLM_TLS_CERT_PATH:-}" && -n "${LLM_TLS_KEY_PATH:-}" ]]; then
  curl_args+=(--cert "${LLM_TLS_CERT_PATH}" --key "${LLM_TLS_KEY_PATH}")
fi

response="$(curl "${curl_args[@]}" "${LLM_ENDPOINT}")" || fail "LLM response request failed"
answer="$(jq -r '.choices[0].message.content // empty' <<<"${response}")"
[[ -n "${answer}" ]] || fail "LLM answer was empty: ${response}"

normalized_answer="$(tr '[:upper:]' '[:lower:]' <<<"${answer}")"
if grep -q "not_found" <<<"${normalized_answer}"; then
  fail "answer should use kernel context instead of NOT_FOUND: ${answer}"
fi

if ! grep -Eq 'maxconnections|max connections|db_max_connections|50 to 5|config map' <<<"${normalized_answer}"; then
  fail "answer did not mention the likely operational cause: ${answer}"
fi

if ! grep -Eq 'rollback|roll back|secondary region|traffic shift|shifted most traffic|shifted traffic' <<<"${normalized_answer}"; then
  fail "answer did not mention the mitigation action: ${answer}"
fi

echo "$(
  jq -n \
    --arg run_id "${run_id}" \
    --arg answer "${answer//$'\n'/ }" \
    --arg rendered_excerpt "$(printf '%s' "${rendered_context}" | head -c 320)" \
    --argjson roundtrip_summary "${summary}" \
    '{
      run_id: $run_id,
      answer: $answer,
      rendered_excerpt: $rendered_excerpt,
      roundtrip_summary: $roundtrip_summary
    }'
)"

pass "async blind context consumption succeeded run_id=${run_id}"
