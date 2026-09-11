#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "==> Preparing WASM test assemblies..."
./scripts/build-orbs.sh

# Check if --coverage was passed
COVERAGE=false
PASSTHROUGH=()

for arg in "$@"; do
    if [ "$arg" = "--coverage" ]; then
        COVERAGE=true
    else
        PASSTHROUGH+=("$arg")
    fi
done

if [ "$COVERAGE" = true ]; then
    echo "==> Running workspace tests with code coverage..."
    if [ ${#PASSTHROUGH[@]} -eq 0 ]; then
        cargo llvm-cov --workspace --exclude hello-wasm --exclude agent-worker --exclude http-client
    else
        cargo llvm-cov --workspace --exclude hello-wasm --exclude agent-worker --exclude http-client "${PASSTHROUGH[@]}"
    fi
else
    echo "==> Running workspace tests..."
    if [ ${#PASSTHROUGH[@]} -eq 0 ]; then
        cargo test --workspace
    else
        cargo test --workspace "${PASSTHROUGH[@]}"
    fi
fi
