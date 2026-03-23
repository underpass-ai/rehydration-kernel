#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "${ROOT_DIR}"

TARGETS=(
  "api/proto/underpass/rehydration/kernel/v1beta1"
  "api/asyncapi/context-projection.v1beta1.yaml"
  "api/examples/kernel/v1beta1"
)

LEGACY_PATTERN='\bcase_id\b|\bstory_id\b|\btask_id\b|\bepic\b|\bproject\b|planning\.|orchestration\.'

if rg -n -P "${LEGACY_PATTERN}" "${TARGETS[@]}"; then
  echo "legacy product-specific nouns leaked into the generic kernel contract" >&2
  exit 1
fi

echo "kernel contract naming policy passed"
