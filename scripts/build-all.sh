#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
SRC_TAURI="src-tauri"

targets=(
  "x86_64-apple-darwin"
  "aarch64-apple-darwin"
  "x86_64-pc-windows-msvc"
  "aarch64-pc-windows-msvc"
)

for t in "${targets[@]}"; do
  echo "=== Building $t ==="
  cargo tauri build --target "$t" || echo "Build for $t failed, continuing..."
done
