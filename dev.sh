#!/bin/bash
# dev.sh — build backend first, then launch Tauri dev mode.
# Always run this instead of `npm run tauri:dev` directly so the
# backend binary stays in sync with its source.
set -e
cd "$(dirname "$0")"

echo "▶ building polish-backend..."
touch crates/backend/src/main.rs   # bust Cargo fingerprint cache
cargo build -p polish-backend

echo "▶ syncing binary to Tauri externalBin..."
# Tauri copies binaries/polish-backend-aarch64-apple-darwin into the build,
# overwriting target/debug/polish-backend. Keep them in sync.
cp target/debug/polish-backend \
   desktop/src-tauri/binaries/polish-backend-aarch64-apple-darwin

echo "▶ launching tauri dev..."
cd desktop
npm run tauri:dev
