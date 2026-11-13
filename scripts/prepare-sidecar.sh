#!/usr/bin/env bash
# Copies a llama-server build into src-tauri/binaries/ under the name Tauri expects
# for sidecars: llama-server-<target-triple>[.exe].
#
# Usage: scripts/prepare-sidecar.sh <dir-with-artifacts> [target-triple]
# The artifacts come from the "Build llama-server sidecar" workflow.
set -euo pipefail

SRC_DIR="${1:?usage: prepare-sidecar.sh <dir-with-artifacts> [target-triple]}"
TRIPLE="${2:-$(rustc -vV | sed -n 's/^host: //p')}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST_DIR="$ROOT/src-tauri/binaries"
EXT=""
case "$TRIPLE" in
  *windows*) EXT=".exe" ;;
esac

SRC="$SRC_DIR/llama-server-$TRIPLE$EXT"
if [ ! -f "$SRC" ]; then
  echo "missing $SRC" >&2
  exit 1
fi

mkdir -p "$DEST_DIR"
cp "$SRC" "$DEST_DIR/llama-server-$TRIPLE$EXT"
chmod +x "$DEST_DIR/llama-server-$TRIPLE$EXT"
echo "placed $DEST_DIR/llama-server-$TRIPLE$EXT"
