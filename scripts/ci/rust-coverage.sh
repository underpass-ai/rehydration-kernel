#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "${ROOT_DIR}"

mkdir -p target/llvm-cov

# Merge coverage from the fast workspace suite plus container-backed adapter
# tests so Sonar sees the real exercised read/write paths.
cargo llvm-cov clean --workspace

cargo llvm-cov --workspace --locked --no-report

. "${ROOT_DIR}/scripts/ci/testcontainers-runtime.sh"

cargo llvm-cov \
  -p rehydration-adapter-valkey \
  --features container-tests \
  --test valkey_integration \
  --locked \
  --no-report

cargo llvm-cov \
  -p rehydration-adapter-neo4j \
  --features container-tests \
  --test neo4j_integration \
  --locked \
  --no-report

cargo llvm-cov \
  -p rehydration-transport-grpc \
  --features container-tests \
  --test compatibility_integration \
  --locked \
  --no-report

cargo llvm-cov report --locked --lcov --output-path target/llvm-cov/lcov.info
