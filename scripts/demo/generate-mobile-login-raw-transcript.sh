#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUTPUT_PATH="${1:-${ROOT_DIR}/docs/research/token-savings/mobile-login-incident/raw_transcript.txt}"
ATTEMPTS="${2:-520}"

mkdir -p "$(dirname "${OUTPUT_PATH}")"

{
  cat <<'HEADER'
MOBILE LOGIN INCIDENT RAW MULTI-AGENT TRANSCRIPT

This file intentionally models the noisy work log a generalist LLM would have
to read if no durable kernel memory existed. The use case is an incident that
requires many failed solution attempts before the effective rollback is chosen.
Every worklog chunk below is one attempted diagnosis or mitigation, with the
agent hypothesis, action, tool output, observed result, and discard rationale.
The compact kernel contexts next to this file are scoped memory reconstructions
of the durable board, not a summary of this raw transcript after the fact.

HEADER

  hypotheses=(
    "mobile retry storm exhausted the session refresh path"
    "Auth API capacity saturation caused token validation timeouts"
    "edge cache served stale JWKS material to iOS clients"
    "iOS clock skew invalidated otherwise correct tokens"
    "jwt-parser-v3 rejected the legacy audience claim shape"
    "feature flag cohort mismatch sent iOS to the wrong auth realm"
    "CDN WAF rule blocked the OAuth callback for mobile Safari"
    "key rotation race left auth-api with mixed verifier state"
    "TLS session resumption changed client fingerprint handling"
    "OAuth redirect URI normalization differed by app version"
    "regional auth dependency degraded only the mobile route"
    "database pool pressure slowed token introspection"
  )

  actions=(
    "enabled stricter mobile retry backoff"
    "scaled Auth API replicas"
    "purged auth edge cache"
    "forced mobile clients to refresh JWKS"
    "disabled jwt-parser-v3 for one canary shard"
    "pinned the old auth realm for iOS cohort"
    "bypassed the WAF rule for OAuth callback"
    "replayed key rotation propagation"
    "disabled TLS session resumption for iOS traffic"
    "normalized redirect URI casing"
    "shifted mobile auth traffic to the secondary region"
    "raised token introspection database pool limits"
  )

  outcomes=(
    "repeated attempts dropped but valid iOS logins still failed"
    "latency stayed healthy but the 401 rate was unchanged"
    "cache hit ratio reset but the iOS-only 401 pattern remained"
    "JWKS refresh completed but rejected legacy audience claims persisted"
    "one shard improved but the broad incident remained unresolved"
    "cohort routing stabilized but login redirects continued"
    "callback traffic passed WAF but Auth API still rejected tokens"
    "verifier state converged but iOS clients still received 401 responses"
    "client fingerprints changed but successful login rate did not recover"
    "redirect URI errors disappeared but login redirects continued"
    "secondary region showed the same iOS-only failure pattern"
    "database wait time improved but token validation still failed"
  )

  agents=(
    "mobile-agent"
    "auth-agent"
    "release-agent"
    "mitigation-agent"
    "infra-agent"
    "observability-agent"
    "qa-agent"
    "coordinator-agent"
  )

  for i in $(seq 1 "${ATTEMPTS}"); do
    minute=$((i % 60))
    shard=$((i % 12))
    request=$((100000 + i))
    auth_401=$((23 + (i % 19)))
    ios_fail=$((31 + (i % 11)))
    hypothesis="${hypotheses[$(( (i - 1) % ${#hypotheses[@]} ))]}"
    action="${actions[$(( (i - 1) % ${#actions[@]} ))]}"
    outcome="${outcomes[$(( (i - 1) % ${#outcomes[@]} ))]}"
    primary_agent="${agents[$(( (i - 1) % ${#agents[@]} ))]}"
    next_agent="${agents[$(( i % ${#agents[@]} ))]}"
    attempt_id="$(printf 'attempt-%04d' "${i}")"

    cat <<EOF
--- FAILED ATTEMPT ${i}: ${attempt_id} ---

[${primary_agent}][10:${minute}] hypothesis
Hypothesis under test: ${hypothesis}.
The agent writes the full reasoning trail into the shared transcript because
there is no durable typed memory boundary in this raw baseline. The same
incident facts are repeated for later agents: iOS users enter valid credentials,
receive a token-shaped response, and land back on the login screen.

[${primary_agent}][action]
Attempt ${attempt_id}: ${action}.
Expected result: iOS login failure rate should fall toward the 2% baseline if
this hypothesis is the active cause.

[mobile-agent][tool-output]
{"attempt":"${attempt_id}","metric":"ios_login_failure_rate","value":"${ios_fail}%","baseline":"2%","window":"5m","shard":"ios-${shard}","request":"mob-${request}"}

[auth-agent][tool-output]
{"attempt":"${attempt_id}","service":"auth-api","status":"401_spike","count_over_baseline":"${auth_401}x","client":"ios","sample_token_shape":"legacy_audience","request":"auth-${request}"}

[${primary_agent}][observed-result]
Observed result: ${outcome}. The attempt is recorded as failed, not discarded,
because later audit needs to know why the team did not stop here.

[${next_agent}][handoff]
Handoff after ${attempt_id}: keep jwt-parser-v3 rollout in the suspect set
because it happened before the first user-facing reports. Keep failed attempts
in the record: retry controls, scale-out, cache/JWKS actions, routing changes,
WAF changes, region shifts, and pool tuning have not explained recovery.

[infra-agent][distractor]
Nearby cache deploy remains visible in the raw transcript. For ${attempt_id},
infra metrics show no broad Android impact and no matching backend latency
increase. The distractor remains because raw transcripts do not enforce scope.

[coordinator-agent][handoff-note]
Current working summary after ${attempt_id}: mobile failures plus Auth API 401s
plus parser rollout still point toward a compatibility regression in
jwt-parser-v3, but the team has not yet accepted rollback until enough failed
attempts make alternatives less likely.

EOF
  done

  cat <<FOOTER
--- FINAL AUDIT SUMMARY ---

Audit-agent reconstructs the incident from the raw transcript:
- failed attempts represented in the raw work log: ${ATTEMPTS}.
- technical board without infra distractor: 15 timeline events.
- board with infra distractor: 16 timeline events.
- trace from retry backoff to recovery: four edges.
- inspect recovery: five incoming links and one evidence item.
FOOTER
} >"${OUTPUT_PATH}"

printf 'wrote %s with %s failed attempts\n' "${OUTPUT_PATH}" "${ATTEMPTS}"
