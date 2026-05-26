#!/usr/bin/env bash
# Common helpers for read-only E2E regeneration preflight scripts.

set -uo pipefail

CHECK_TOTAL=0
CHECK_OK=0
CHECK_WARN=0
CHECK_FAIL=0
VERBOSE=${VERBOSE:-0}
JSON_OUTPUT=${JSON_OUTPUT:-0}
ALLOW_REDEPLOY=${ALLOW_REDEPLOY:-0}
ALLOW_REINSTALL_ADAPTERS=${ALLOW_REINSTALL_ADAPTERS:-0}

json_escape() {
  local value=${1-}
  value=${value//\\/\\\\}
  value=${value//"/\\"}
  value=${value//$'\n'/ }
  value=${value//$'\r'/ }
  printf '%s' "$value"
}

print_step_result() {
  local step=$1
  local status=$2
  local message=$3
  CHECK_TOTAL=$((CHECK_TOTAL + 1))
  case "$status" in
    OK) CHECK_OK=$((CHECK_OK + 1)) ;;
    WARN) CHECK_WARN=$((CHECK_WARN + 1)) ;;
    FAIL) CHECK_FAIL=$((CHECK_FAIL + 1)) ;;
    *) status=FAIL; CHECK_FAIL=$((CHECK_FAIL + 1)); message="invalid status from check: $message" ;;
  esac

  if [[ "$JSON_OUTPUT" == "1" ]]; then
    printf '{"status":"%s","step":"%s","message":"%s"}\n' \
      "$(json_escape "$status")" "$(json_escape "$step")" "$(json_escape "$message")"
  else
    printf '[%-4s] %-36s %s\n' "$status" "$step" "$message"
  fi
}

ok() { print_step_result "$1" OK "$2"; }
warn() { print_step_result "$1" WARN "$2"; }
fail() { print_step_result "$1" FAIL "$2"; }

finish_summary() {
  local passed=$CHECK_OK
  if [[ "$JSON_OUTPUT" == "1" ]]; then
    printf '{"summary":{"passed":%d,"total":%d,"warn":%d,"fail":%d}}\n' \
      "$passed" "$CHECK_TOTAL" "$CHECK_WARN" "$CHECK_FAIL"
  else
    printf '%d/%d checks passed' "$passed" "$CHECK_TOTAL"
    if [[ "$CHECK_WARN" -gt 0 || "$CHECK_FAIL" -gt 0 ]]; then
      printf ' (%d warn, %d fail)' "$CHECK_WARN" "$CHECK_FAIL"
    fi
    printf '\n'
  fi

  if [[ "$CHECK_FAIL" -gt 0 ]]; then
    exit 1
  fi
  if [[ "$CHECK_WARN" -gt 0 ]]; then
    exit 2
  fi
  exit 0
}

need_cmd() {
  local step=$1
  local cmd=$2
  if command -v "$cmd" >/dev/null 2>&1; then
    ok "$step" "$cmd available at $(command -v "$cmd")"
    return 0
  fi
  fail "$step" "$cmd not found; install it or adjust PATH"
  return 1
}

run_checked() {
  local step=$1
  local message=$2
  shift 2
  local output
  if output=$("$@" 2>&1); then
    if [[ "$VERBOSE" == "1" && -n "$output" ]]; then
      printf '%s\n' "$output"
    fi
    ok "$step" "$message"
    return 0
  fi
  fail "$step" "$message failed: $output"
  return 1
}

check_branch_state() {
  local repo_path=$1
  local step=${2:-branch state}
  local status
  local head
  local diff_stat
  status=$(git -C "$repo_path" status -sb 2>&1) || { fail "$step" "git status failed: $status"; return 1; }
  head=$(git -C "$repo_path" log -1 --oneline 2>&1) || { fail "$step" "git log failed: $head"; return 1; }
  diff_stat=$(git -C "$repo_path" diff --stat 2>&1) || { fail "$step" "git diff --stat failed: $diff_stat"; return 1; }
  if [[ "$VERBOSE" == "1" ]]; then
    printf '%s\n%s\n%s\n' "$status" "$head" "$diff_stat"
  fi
  if [[ -n $(git -C "$repo_path" status --porcelain) ]]; then
    fail "$step" "working tree dirty at $repo_path; commit/stash before e2e regen"
    return 1
  fi
  ok "$step" "clean tree at $head"
  return 0
}

check_binary_freshness() {
  local binary=$1
  local repo_path=$2
  local step=${3:-binary freshness}
  local binary_path=$binary
  if [[ "$binary" != */* ]]; then
    binary_path=$(command -v "$binary" 2>/dev/null || true)
  fi
  if [[ -z "$binary_path" || ! -x "$binary_path" ]]; then
    fail "$step" "$binary not found or not executable; run cargo install/build step"
    return 1
  fi
  local bin_epoch
  local head_epoch
  bin_epoch=$(stat -c '%Y' "$binary_path" 2>&1) || { fail "$step" "stat failed for $binary_path: $bin_epoch"; return 1; }
  head_epoch=$(git -C "$repo_path" log -1 --format='%ct' HEAD 2>&1) || { fail "$step" "git log timestamp failed: $head_epoch"; return 1; }
  if (( bin_epoch < head_epoch )); then
    fail "$step" "$binary_path is older than HEAD; reinstall from current source"
    return 1
  fi
  ok "$step" "$binary_path is newer than or equal to HEAD"
  return 0
}

check_mtls_cert() {
  local cert_path=$1
  local step=${2:-mTLS cert}
  if [[ ! -s "$cert_path" ]]; then
    fail "$step" "$cert_path missing or empty"
    return 1
  fi
  local dates
  dates=$(openssl x509 -in "$cert_path" -noout -dates 2>&1) || { fail "$step" "openssl could not read $cert_path: $dates"; return 1; }
  if ! openssl x509 -in "$cert_path" -noout -checkend 0 >/dev/null 2>&1; then
    fail "$step" "$cert_path is expired: $dates"
    return 1
  fi
  ok "$step" "$cert_path valid: $(printf '%s' "$dates" | tr '\n' ' ')"
  return 0
}

check_endpoint_model() {
  local url=$1
  local cert=$2
  local key=$3
  local expected_model=$4
  local step=${5:-endpoint model}
  local body
  body=$(curl -sSfk --cert "$cert" --key "$key" "$url" 2>&1) || { fail "$step" "curl failed for $url: $body"; return 1; }
  if command -v jq >/dev/null 2>&1; then
    if ! printf '%s' "$body" | jq -e --arg id "$expected_model" '.data[]?.id == $id' >/dev/null 2>&1; then
      fail "$step" "$expected_model not listed by $url"
      return 1
    fi
  elif ! printf '%s' "$body" | grep -Fq "$expected_model"; then
    fail "$step" "$expected_model not found in response from $url"
    return 1
  fi
  ok "$step" "$expected_model listed by $url"
  return 0
}

first_running_pod() {
  local namespace=$1
  local selector=$2
  kubectl -n "$namespace" get pods -l "$selector" \
    --field-selector=status.phase=Running \
    -o jsonpath='{.items[0].metadata.name}' 2>/dev/null
}

check_adapter_sha() {
  local namespace=$1
  local selector=$2
  local adapter_path=$3
  local expected_sha=$4
  local step=${5:-adapter sha}
  local pod
  pod=$(first_running_pod "$namespace" "$selector")
  if [[ -z "$pod" ]]; then
    fail "$step" "no running pod found for selector $selector in $namespace"
    return 1
  fi
  local output
  output=$(kubectl -n "$namespace" exec "$pod" -- sha256sum "$adapter_path" 2>&1) || { fail "$step" "sha256sum failed in $pod: $output"; return 1; }
  local actual=${output%% *}
  if [[ "$actual" != "$expected_sha" ]]; then
    fail "$step" "adapter SHA mismatch in $pod: expected $expected_sha got $actual"
    return 1
  fi
  ok "$step" "adapter SHA verified in $pod"
  return 0
}

check_kubectl_pod_image() {
  local namespace=$1
  local selector=$2
  local expected=$3
  local step=${4:-pod image}
  local images
  images=$(kubectl -n "$namespace" get pods -l "$selector" -o jsonpath='{range .items[*]}{.metadata.name}{" "}{range .spec.containers[*]}{.image}{" "}{end}{"\n"}{end}' 2>&1) || { fail "$step" "kubectl image query failed: $images"; return 1; }
  if [[ -z "$images" ]]; then
    fail "$step" "no pods found for selector $selector in $namespace"
    return 1
  fi
  if [[ -n "$expected" && "$images" != *"$expected"* ]]; then
    fail "$step" "expected image/tag '$expected' not found; got: $images"
    return 1
  fi
  ok "$step" "pod image check passed for selector $selector"
  if [[ "$VERBOSE" == "1" ]]; then
    printf '%s\n' "$images"
  fi
  return 0
}

check_helm_release_revision() {
  local release=$1
  local namespace=$2
  local min_revision=${3:-1}
  local step=${4:-helm release}
  local line
  line=$(helm list -n "$namespace" 2>/dev/null | awk -v rel="$release" '$1 == rel {print $0}')
  if [[ -z "$line" ]]; then
    fail "$step" "Helm release $release not found in namespace $namespace"
    return 1
  fi
  local revision
  revision=$(awk '{print $3}' <<<"$line")
  if [[ ! "$revision" =~ ^[0-9]+$ || "$revision" -lt "$min_revision" ]]; then
    fail "$step" "release $release revision $revision < expected $min_revision"
    return 1
  fi
  ok "$step" "release $release revision $revision present"
  return 0
}

parse_common_flags() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --redeploy) ALLOW_REDEPLOY=1 ;;
      --reinstall-adapters) ALLOW_REINSTALL_ADAPTERS=1 ;;
      --verbose) VERBOSE=1 ;;
      --json) JSON_OUTPUT=1 ;;
      -h|--help)
        cat <<'USAGE'
Usage: ./scripts/e2e/regen.sh [--verbose] [--json] [--redeploy] [--reinstall-adapters]

Default mode is read-only except for local cargo build/install steps.
--redeploy and --reinstall-adapters are explicit gates for future mutating operations.
USAGE
        exit 0
        ;;
      *)
        fail "argument parsing" "unknown flag: $1"
        finish_summary
        ;;
    esac
    shift
  done
  export VERBOSE JSON_OUTPUT ALLOW_REDEPLOY ALLOW_REINSTALL_ADAPTERS
}
