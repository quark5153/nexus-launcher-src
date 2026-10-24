#!/usr/bin/env bash
# Build Nexus Launcher (3-layer monorepo) and package it for CachyOS / Arch.
#   1) apps/app-frontend  → Vue 3 UI        (npm + vite)
#   2) apps/app           → Tauri v2 shell  (Rust, thin)
#   3) packages/nexus-core → headless engine (Rust, no Tauri)
# Usage: ./build-package.sh [--skip-build]
set -euo pipefail
cd "$(dirname "$0")"

PKGVER=$(python3 -c "import json;print(json.load(open('apps/app/tauri.conf.json'))['version'])")
OUT="dist-packages"

if [[ "${1:-}" != "--skip-build" ]]; then
  echo "==> [1/3] Frontend (Vue 3 + Vite)"
  npm --prefix apps/app-frontend run build
  echo "==> [2/3] + [3/3] Rust shell + nexus-core (this takes a few minutes)"
  cargo build --release -p nexus-shell --features custom-protocol
fi

BIN="target/release/nexus-launcher"
[[ -x "$BIN" ]] || { echo "missing binary: $BIN" >&2; exit 1; }

echo "==> Staging package files"
cp "$BIN" packaging/nexus-launcher
magick icon-source.png -resize 512x512 packaging/nexus-launcher.png

echo "==> makepkg"
(cd packaging && makepkg -f --nodeps --nocheck)

mkdir -p "$OUT"
PKGFILE=$(ls -t packaging/nexus-launcher-"$PKGVER"-*.pkg.tar.zst | head -1)
cp "$PKGFILE" "$OUT/"
echo "==> Package ready: $OUT/$(basename "$PKGFILE")"
echo "    Install with: sudo pacman -U $(basename "$PKGFILE")"
