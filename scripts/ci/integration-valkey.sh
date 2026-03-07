#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONTAINER_RUNTIME="${CONTAINER_RUNTIME:-auto}"
PODMAN_SERVICE_PID=""
PODMAN_SOCKET_PATH=""
PODMAN_LOG_PATH=""

cd "${ROOT_DIR}"

cleanup() {
  if [ -n "${PODMAN_SERVICE_PID}" ] && kill -0 "${PODMAN_SERVICE_PID}" >/dev/null 2>&1; then
    kill "${PODMAN_SERVICE_PID}" >/dev/null 2>&1 || true
    wait "${PODMAN_SERVICE_PID}" 2>/dev/null || true
  fi
}
trap cleanup EXIT

use_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    return 1
  fi

  docker info >/dev/null
}

setup_podman() {
  if ! command -v podman >/dev/null 2>&1; then
    echo "podman is not installed" >&2
    return 1
  fi

  export TESTCONTAINERS_RYUK_DISABLED=true

  if [ -z "${DOCKER_HOST:-}" ]; then
    local default_socket
    default_socket="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/podman/podman.sock"

    if [ -S "${default_socket}" ]; then
      export DOCKER_HOST="unix://${default_socket}"
      return 0
    fi

    if command -v systemctl >/dev/null 2>&1; then
      systemctl --user start podman.socket >/dev/null 2>&1 || true
      if [ -S "${default_socket}" ]; then
        export DOCKER_HOST="unix://${default_socket}"
        return 0
      fi
    fi

    if [ -z "${XDG_RUNTIME_DIR:-}" ] || [ ! -w "${XDG_RUNTIME_DIR}" ]; then
      export XDG_RUNTIME_DIR="${TMPDIR:-/tmp}/podman-runtime-${UID}"
    fi
    mkdir -p "${XDG_RUNTIME_DIR}"

    PODMAN_SOCKET_PATH="$(mktemp -u "${TMPDIR:-/tmp}/podman-testcontainers.XXXXXX.sock")"
    PODMAN_LOG_PATH="${TMPDIR:-/tmp}/podman-testcontainers.log"
    rm -f "${PODMAN_SOCKET_PATH}" "${PODMAN_LOG_PATH}"

    podman system service --time=0 "unix://${PODMAN_SOCKET_PATH}" >"${PODMAN_LOG_PATH}" 2>&1 &
    PODMAN_SERVICE_PID=$!

    for _ in $(seq 1 50); do
      if [ -S "${PODMAN_SOCKET_PATH}" ]; then
        export DOCKER_HOST="unix://${PODMAN_SOCKET_PATH}"
        return 0
      fi
      sleep 0.2
    done

    echo "podman socket did not become available" >&2
    cat "${PODMAN_LOG_PATH}" >&2 || true
    return 1
  fi

  return 0
}

select_runtime() {
  case "${CONTAINER_RUNTIME}" in
    auto)
      if use_docker; then
        return 0
      fi
      setup_podman
      ;;
    docker)
      use_docker
      ;;
    podman)
      setup_podman
      ;;
    *)
      echo "unsupported CONTAINER_RUNTIME=${CONTAINER_RUNTIME}; expected auto, docker, or podman" >&2
      return 1
      ;;
  esac
}

select_runtime

cargo test \
  -p rehydration-adapter-valkey \
  --features container-tests \
  --test valkey_integration \
  --locked \
  -- \
  --nocapture
