#!/usr/bin/env bash
set -euo pipefail
source /app/common.sh

echo "=== async-vllm-spine-ab-comparison ==="

require_env LLM_ENDPOINT
require_env LLM_MODEL
require_env LLM_PROVIDER

if [[ "${LLM_PROVIDER}" != "openai" ]]; then
  fail "async-vllm-spine-ab-comparison requires LLM_PROVIDER=openai"
fi

run_id="${RUN_ID:-e2e-vllm-spine-ab-$(date +%s)}"
legacy_input="${LEGACY_GRAPH_BATCH_INPUT:-/app/api/examples/kernel/v1beta1/async/pir-sequential-spine.legacy-batch.json}"
spine_input="${GRAPH_RELATION_INPUT:-/app/api/examples/kernel/v1beta1/async/pir-sequential-spine.relation-roundtrip.json}"
question="${SPINE_AB_QUESTION:-Answer in one concise sentence: which planned change addresses the cache-stampede finding, and what in the context explicitly proves or disproves that direct link? If the context does not explicitly prove the direct link, say so.}"

[[ -f "${legacy_input}" ]] || fail "legacy GraphBatch fixture not found: ${legacy_input}"
[[ -f "${spine_input}" ]] || fail "relation fixture not found: ${spine_input}"

export PIR_GRAPH_BATCH_INCLUDE_RENDERED_CONTENT=true

strip_markdown_fences() {
  local content="$1"
  printf '%s\n' "${content}" | awk '
    BEGIN { in_fence = 0 }
    /^```/ {
      if (in_fence == 0) { in_fence = 1; next }
      in_fence = 0; next
    }
    { print }
  '
}

build_chat_request_body() {
  local model="$1"
  local prompt="$2"
  jq -n \
    --arg model "${model}" \
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
}

call_chat_completion() {
  local endpoint="$1"
  local model="$2"
  local api_key="${3:-}"
  local cert_path="${4:-}"
  local key_path="${5:-}"
  local prompt="$6"
  local body
  local response
  local answer
  local -a curl_args

  body="$(build_chat_request_body "${model}" "${prompt}")"
  curl_args=(
    -sS
    -H "content-type: application/json"
    --data "${body}"
  )

  if [[ -n "${api_key}" ]]; then
    curl_args+=(-H "authorization: Bearer ${api_key}")
  fi
  if [[ -n "${cert_path}" && -n "${key_path}" ]]; then
    curl_args+=(--cert "${cert_path}" --key "${key_path}")
  fi

  response="$(curl "${curl_args[@]}" "${endpoint}")" || fail "chat completion request failed: ${endpoint}"
  answer="$(jq -r '.choices[0].message.content // empty' <<<"${response}")"
  [[ -n "${answer}" ]] || fail "LLM answer was empty: ${response}"
  printf '%s' "${answer}"
}

judge_answer() {
  local variant="$1"
  local rendered_context="$2"
  local answer="$3"
  local judge_endpoint="${LLM_JUDGE_ENDPOINT:-${LLM_ENDPOINT}}"
  local judge_model="${LLM_JUDGE_MODEL:-${LLM_MODEL}}"
  local judge_api_key="${LLM_JUDGE_API_KEY:-${LLM_API_KEY:-}}"
  local judge_provider="${LLM_JUDGE_PROVIDER:-${LLM_PROVIDER}}"
  local prompt
  local raw
  local stripped

  if [[ "${judge_provider}" != "openai" ]]; then
    fail "async-vllm-spine-ab-comparison requires an OpenAI-compatible judge endpoint"
  fi

  prompt="$(
    jq -rn \
      --arg variant "${variant}" \
      --arg question "${question}" \
      --arg rendered_context "${rendered_context}" \
      --arg answer "${answer}" \
      '
        "You are scoring answer quality for a kernel context A/B comparison.\n\n" +
        "Variant: " + $variant + "\n" +
        "Question: " + $question + "\n\n" +
        "Context:\n" + $rendered_context + "\n\n" +
        "Answer:\n" + $answer + "\n\n" +
        "Return strict JSON with keys " +
        "\"change_correct\", \"grounded\", \"direct_link_handled_correctly\", \"score\", and \"reason\".\n" +
        "Scoring rubric:\n" +
        "- legacy variant: the answer can score at most 3. The correct grounded answer names the change as enabling cache jitter and states that the context does not explicitly prove a direct link from the decision to the finding.\n" +
        "- spine variant: the answer should score 4 only if it names the change as enabling cache jitter and cites explicit proof from context that the decision directly addresses the finding, such as the ADDRESSES relation or its rationale.\n" +
        "- score 2 or lower if the answer invents proof, misses the change, or is not grounded.\n" +
        "- direct_link_handled_correctly must be true when the answer treats the explicit-link question correctly for the variant.\n" +
        "- grounded must be true only if the answer is explicitly supported by the context."'
  )"

  raw="$(call_chat_completion "${judge_endpoint}" "${judge_model}" "${judge_api_key}" "${LLM_JUDGE_TLS_CERT_PATH:-}" "${LLM_JUDGE_TLS_KEY_PATH:-}" "${prompt}")"
  stripped="$(strip_markdown_fences "${raw}")"
  jq -e '.' >/dev/null <<<"${stripped}" || fail "judge did not return valid JSON: ${raw}"
  printf '%s' "${stripped}"
}

run_legacy_roundtrip() {
  local run_id="$1"
  local summary
  local rendered_context
  local prompt
  local answer
  local verdict
  local roundtrip_cmd=()

  build_graph_batch_roundtrip_command roundtrip_cmd "${legacy_input}" "${run_id}"
  summary="$("${roundtrip_cmd[@]}")" || fail "legacy GraphBatch roundtrip failed"
  assert_roundtrip_summary "${summary}"
  rendered_context="$(jq -r '.rendered_content // empty' <<<"${summary}")"
  [[ -n "${rendered_context}" ]] || fail "legacy roundtrip summary did not include rendered_content"
  prompt="${question}"$'\n\n'"Context:\n${rendered_context}"
  answer="$(call_chat_completion "${LLM_ENDPOINT}" "${LLM_MODEL}" "${LLM_API_KEY:-}" "${LLM_TLS_CERT_PATH:-}" "${LLM_TLS_KEY_PATH:-}" "${prompt}")"
  verdict="$(judge_answer legacy "${rendered_context}" "${answer}")"

  jq -n \
    --arg variant legacy \
    --arg answer "${answer//$'\n'/ }" \
    --argjson roundtrip_summary "${summary}" \
    --argjson judge "${verdict}" \
    '{
      variant: $variant,
      answer: $answer,
      roundtrip_summary: $roundtrip_summary,
      judge: $judge
    }'
}

run_spine_roundtrip() {
  local run_id="$1"
  local summary
  local rendered_context
  local prompt
  local answer
  local verdict
  local roundtrip_cmd=()

  build_graph_relation_roundtrip_command roundtrip_cmd "${spine_input}" "${run_id}"
  summary="$("${roundtrip_cmd[@]}")" || fail "spine relation roundtrip failed"
  assert_roundtrip_summary "${summary}"
  rendered_context="$(jq -r '.rendered_content // empty' <<<"${summary}")"
  [[ -n "${rendered_context}" ]] || fail "spine roundtrip summary did not include rendered_content"
  prompt="${question}"$'\n\n'"Context:\n${rendered_context}"
  answer="$(call_chat_completion "${LLM_ENDPOINT}" "${LLM_MODEL}" "${LLM_API_KEY:-}" "${LLM_TLS_CERT_PATH:-}" "${LLM_TLS_KEY_PATH:-}" "${prompt}")"
  verdict="$(judge_answer spine "${rendered_context}" "${answer}")"

  jq -n \
    --arg variant spine \
    --arg answer "${answer//$'\n'/ }" \
    --argjson roundtrip_summary "${summary}" \
    --argjson judge "${verdict}" \
    '{
      variant: $variant,
      answer: $answer,
      roundtrip_summary: $roundtrip_summary,
      judge: $judge
    }'
}

legacy_result="$(run_legacy_roundtrip "${run_id}-legacy")"
spine_result="$(run_spine_roundtrip "${run_id}-spine")"

legacy_score="$(jq -r '.judge.score' <<<"${legacy_result}")"
spine_score="$(jq -r '.judge.score' <<<"${spine_result}")"
spine_change_correct="$(jq -r '.judge.change_correct' <<<"${spine_result}")"
spine_link_ok="$(jq -r '.judge.direct_link_handled_correctly' <<<"${spine_result}")"
spine_grounded="$(jq -r '.judge.grounded' <<<"${spine_result}")"

[[ "${spine_change_correct}" == "true" ]] || fail "spine answer did not identify the change correctly: $(jq -r '.answer' <<<"${spine_result}")"
[[ "${spine_link_ok}" == "true" ]] || fail "spine answer handled the explicit-link question incorrectly: $(jq -r '.judge.reason' <<<"${spine_result}")"
[[ "${spine_grounded}" == "true" ]] || fail "spine answer was not grounded: $(jq -r '.judge.reason' <<<"${spine_result}")"
(( spine_score > legacy_score )) || fail "expected spine score to exceed legacy score, got legacy=${legacy_score} spine=${spine_score}"

echo "$(
  jq -n \
    --arg run_id "${run_id}" \
    --arg question "${question}" \
    --argjson legacy "${legacy_result}" \
    --argjson spine "${spine_result}" \
    '{
      run_id: $run_id,
      question: $question,
      legacy: $legacy,
      spine: $spine,
      improvement: {
        legacy_score: $legacy.judge.score,
        spine_score: $spine.judge.score,
        score_delta: ($spine.judge.score - $legacy.judge.score)
      }
    }'
)"

pass "async vLLM spine A/B comparison succeeded legacy_score=${legacy_score} spine_score=${spine_score}"
