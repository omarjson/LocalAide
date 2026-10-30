#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
SRC_TAURI="$REPO_ROOT/src-tauri"

echo "=== Tauri Packaging Verification ==="
echo "Repo root: $REPO_ROOT"

# Check Rust version
RUST_VERSION=$(rustc --version | awk '{print $2}')
echo "Rust version: $RUST_VERSION"

# Check Tauri config
if [[ -f "$SRC_TAURI/tauri.conf.json" ]]; then
  echo "tauri.conf.json present"
  grep -E '"bundle"|"beforeBuildCommand"' "$SRC_TAURI/tauri.conf.json" || true
else
  echo "ERROR: tauri.conf.json missing" >&2
  exit 1
fi

# Check targets
echo "Installed targets:"
rustup target list --installed | grep -E 'x86_64-pc-windows-msvc|x86_64-apple-darwin' || true

# Verify frontend build exists
if [[ -d "$REPO_ROOT/dist" ]]; then
  echo "Frontend dist present"
  ls -lh "$REPO_ROOT/dist/assets" | head
else
  echo "WARNING: frontend dist missing – run npm run build first"
fi

# Verify models dir structure
MODELS_DIR="$HOME/.aiassistant/models"
echo "Models dir: $MODELS_DIR"
if [[ -d "$MODELS_DIR" ]]; then
  find "$MODELS_DIR" -maxdepth 2 -type d | head
else
  echo "Models dir not yet created"
fi

echo ""
echo "Build commands:"
echo "  Windows NSIS (x86_64): cd $SRC_TAURI && cargo tauri build --target x86_64-pc-windows-msvc"
echo "  macOS dmg (Intel):     cd $SRC_TAURI && cargo tauri build --target x86_64-apple-darwin"
echo ""
echo "Outputs will be in:"
echo "  Windows: $SRC_TAURI/target/x86_64-pc-windows-msvc/release/bundle/nsis/"
echo "  macOS:   $SRC_TAURI/target/x86_64-apple-darwin/release/bundle/dmg/"
echo ""
echo "SHA256 verification:"
echo "  Models are SHA256 verified on download via model_manager::download."
echo "  To verify an installed model: use model_verify command."
echo ""
echo "Verification complete."
