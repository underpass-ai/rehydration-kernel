#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "${ROOT_DIR}"

mkdir -p target/llvm-cov

cargo llvm-cov --workspace --locked --lcov --output-path target/llvm-cov/lcov.info
