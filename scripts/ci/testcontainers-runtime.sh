#!/usr/bin/env bash

set -euo pipefail

: "${CONTAINER_RUNTIME:=auto}"
: "${TESTCONTAINERS_RUNTIME_INITIALIZED:=0}"

if [[ "${TESTCONTAINERS_RUNTIME_INITIALIZED}" == "1" ]]; then
  return 0
fi

TESTCONTAINERS_PODMAN_SERVICE_PID=""
TESTCONTAINERS_PODMAN_SOCKET_PATH=""
TESTCONTAINERS_PODMAN_LOG_PATH=""

testcontainers_cleanup_runtime() {
  if [[ -n "${TESTCONTAINERS_PODMAN_SERVICE_PID}" ]] && kill -0 "${TESTCONTAINERS_PODMAN_SERVICE_PID}" >/dev/null 2>&1; then
    kill "${TESTCONTAINERS_PODMAN_SERVICE_PID}" >/dev/null 2>&1 || true
    wait "${TESTCONTAINERS_PODMAN_SERVICE_PID}" 2>/dev/null || true
  fi

  return 0
}

testcontainers_use_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    return 1
  fi

  docker info >/dev/null
}

testcontainers_setup_podman() {
  if ! command -v podman >/dev/null 2>&1; then
    echo "podman is not installed" >&2
    return 1
  fi

  export TESTCONTAINERS_RYUK_DISABLED=true

  if [[ -z "${DOCKER_HOST:-}" ]]; then
    local default_socket
    default_socket="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/podman/podman.sock"

    if [[ -S "${default_socket}" ]]; then
      export DOCKER_HOST="unix://${default_socket}"
      return 0
    fi

    if command -v systemctl >/dev/null 2>&1; then
      systemctl --user start podman.socket >/dev/null 2>&1 || true
      if [[ -S "${default_socket}" ]]; then
        export DOCKER_HOST="unix://${default_socket}"
        return 0
      fi
    fi

    if [[ -z "${XDG_RUNTIME_DIR:-}" || ! -w "${XDG_RUNTIME_DIR}" ]]; then
      export XDG_RUNTIME_DIR="${TMPDIR:-/tmp}/podman-runtime-${UID}"
    fi
    mkdir -p "${XDG_RUNTIME_DIR}"

    TESTCONTAINERS_PODMAN_SOCKET_PATH="$(mktemp -u "${TMPDIR:-/tmp}/podman-testcontainers.XXXXXX.sock")"
    TESTCONTAINERS_PODMAN_LOG_PATH="${TMPDIR:-/tmp}/podman-testcontainers.log"
    rm -f "${TESTCONTAINERS_PODMAN_SOCKET_PATH}" "${TESTCONTAINERS_PODMAN_LOG_PATH}"

    podman system service --time=0 "unix://${TESTCONTAINERS_PODMAN_SOCKET_PATH}" >"${TESTCONTAINERS_PODMAN_LOG_PATH}" 2>&1 &
    TESTCONTAINERS_PODMAN_SERVICE_PID=$!

    for _ in $(seq 1 50); do
      if [[ -S "${TESTCONTAINERS_PODMAN_SOCKET_PATH}" ]]; then
        export DOCKER_HOST="unix://${TESTCONTAINERS_PODMAN_SOCKET_PATH}"
        return 0
      fi
      sleep 0.2
    done

    echo "podman socket did not become available" >&2
    cat "${TESTCONTAINERS_PODMAN_LOG_PATH}" >&2 || true
    return 1
  fi

  return 0
}

testcontainers_select_runtime() {
  case "${CONTAINER_RUNTIME}" in
    auto)
      if testcontainers_use_docker; then
        return 0
      fi
      testcontainers_setup_podman
      ;;
    docker)
      testcontainers_use_docker
      ;;
    podman)
      testcontainers_setup_podman
      ;;
    *)
      echo "unsupported CONTAINER_RUNTIME=${CONTAINER_RUNTIME}; expected auto, docker, or podman" >&2
      return 1
      ;;
  esac
}

testcontainers_select_runtime
trap testcontainers_cleanup_runtime EXIT
export TESTCONTAINERS_RUNTIME_INITIALIZED=1
