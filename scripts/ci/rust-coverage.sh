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

run_container_coverage_test() {
  local package="$1"
  local test_target="$2"

  # Coverage instrumentation makes the container-backed suites slower and more
  # sensitive to parallel startup/load spikes, so keep them single-threaded.
  RUST_TEST_THREADS=1 cargo llvm-cov \
    -p "${package}" \
    --features container-tests \
    --test "${test_target}" \
    --locked \
    --no-report \
    -- \
    --test-threads=1
}

run_container_coverage_test rehydration-adapter-valkey valkey_integration
run_container_coverage_test rehydration-adapter-neo4j neo4j_integration
run_container_coverage_test rehydration-transport-grpc compatibility_integration

cargo llvm-cov report --locked --lcov --output-path target/llvm-cov/lcov.info
