#!/usr/bin/env bash
set -euo pipefail

CHART_PATH="${1:-charts/rehydration-kernel}"

helm lint "${CHART_PATH}"
helm template rehydration-kernel "${CHART_PATH}" >/tmp/rehydration-kernel-helm-template.yaml
