#!/usr/bin/env bash
set -euo pipefail

# Ensure we run from the workspace root
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "==> Checking for wasm32-wasip2 target..."
if ! rustup target list --installed | grep -q "wasm32-wasip2"; then
    echo "==> Installing wasm32-wasip2 target via rustup..."
    rustup target add wasm32-wasip2
fi

echo "==> Building WASM orbs (wasm32-wasip2)..."
cargo build -p hello-wasm -p agent-worker -p http-client --target wasm32-wasip2 "$@"
echo "==> WASM orbs built successfully in target/wasm32-wasip2/debug/"
