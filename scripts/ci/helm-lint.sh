#!/usr/bin/env bash
set -euo pipefail

CHART_PATH="${1:-charts/rehydration-kernel}"
DEV_VALUES="${CHART_PATH}/values.dev.yaml"
DEFAULT_ERR="${TMPDIR:-/tmp}/rehydration-kernel-helm-default.err"

helm lint "${CHART_PATH}" -f "${DEV_VALUES}"
helm template rehydration-kernel "${CHART_PATH}" -f "${DEV_VALUES}" >/tmp/rehydration-kernel-helm-template.yaml

if helm template rehydration-kernel "${CHART_PATH}" > /dev/null 2>"${DEFAULT_ERR}"; then
  echo "default chart render unexpectedly succeeded" >&2
  exit 1
fi

grep -q "set image.tag or image.digest" "${DEFAULT_ERR}"
