#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OVERLAY_FILE="${ROOT_DIR}/compose/starship-demo.compose.yaml"
BASE_COMPOSE_FILE="${BASE_COMPOSE_FILE:-}"
CONTAINER_RUNTIME="${CONTAINER_RUNTIME:-auto}"

if [[ -z "${BASE_COMPOSE_FILE}" ]]; then
  if [[ -f "${ROOT_DIR}/docker-compose.yml" ]]; then
    BASE_COMPOSE_FILE="${ROOT_DIR}/docker-compose.yml"
  else
    echo "BASE_COMPOSE_FILE is required when no ./docker-compose.yml exists" >&2
    exit 1
  fi
fi

resolve_compose_command() {
  case "${CONTAINER_RUNTIME}" in
    docker)
      echo "docker compose"
      ;;
    podman)
      echo "podman compose"
      ;;
    auto)
      if command -v docker >/dev/null 2>&1; then
        echo "docker compose"
      elif command -v podman >/dev/null 2>&1; then
        echo "podman compose"
      else
        echo "missing docker or podman for compose execution" >&2
        exit 1
      fi
      ;;
    *)
      echo "unsupported CONTAINER_RUNTIME: ${CONTAINER_RUNTIME}" >&2
      exit 1
      ;;
  esac
}

compose_command="$(resolve_compose_command)"
read -r -a compose_parts <<< "${compose_command}"

"${compose_parts[@]}" \
  -f "${BASE_COMPOSE_FILE}" \
  -f "${OVERLAY_FILE}" \
  run --rm starship-demo-runner
