#!/usr/bin/env bash
set -euo pipefail

if command -v protoc >/dev/null 2>&1; then
  protoc --version
  exit 0
fi

for required in curl unzip; do
  if ! command -v "${required}" >/dev/null 2>&1; then
    echo "protoc installer requires ${required}" >&2
    exit 1
  fi
done

case "$(uname -m)" in
  x86_64 | amd64)
    protoc_arch="x86_64"
    ;;
  aarch64 | arm64)
    protoc_arch="aarch_64"
    ;;
  *)
    echo "unsupported protoc installer architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

PROTOC_VERSION="${PROTOC_VERSION:-25.3}"
PROTOC_PLATFORM="linux-${protoc_arch}"
PROTOC_ZIP="protoc-${PROTOC_VERSION}-${PROTOC_PLATFORM}.zip"
PROTOC_URL="https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/${PROTOC_ZIP}"
PROTOC_INSTALL_DIR="${PROTOC_INSTALL_DIR:-${RUNNER_TEMP:-/tmp}/protoc-${PROTOC_VERSION}}"

rm -rf "${PROTOC_INSTALL_DIR}"
mkdir -p "${PROTOC_INSTALL_DIR}"

curl --fail --location --show-error --silent \
  --retry 3 --retry-delay 2 --connect-timeout 20 --max-time 120 \
  --output "${PROTOC_INSTALL_DIR}/${PROTOC_ZIP}" \
  "${PROTOC_URL}"

unzip -q "${PROTOC_INSTALL_DIR}/${PROTOC_ZIP}" -d "${PROTOC_INSTALL_DIR}"

export PATH="${PROTOC_INSTALL_DIR}/bin:${PATH}"

if [[ -n "${GITHUB_PATH:-}" ]]; then
  echo "${PROTOC_INSTALL_DIR}/bin" >>"${GITHUB_PATH}"
fi

protoc --version
