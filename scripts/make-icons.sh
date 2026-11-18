#!/usr/bin/env bash
# Regenerates every icon from assets/*.svg. Needs rsvg-convert (librsvg) and the Tauri CLI
# (npm ci first). Output is committed, so normal builds do not run this.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

rsvg-convert -w 1024 -h 1024 assets/icon.svg -o "$TMP/icon-1024.png"
npx --no-install tauri icon "$TMP/icon-1024.png" -o src-tauri/icons >/dev/null
# Keep only the desktop icons this project uses.
find src-tauri/icons -mindepth 1 -maxdepth 1 \
  ! -name '32x32.png' ! -name '128x128.png' ! -name '128x128@2x.png' ! -name 'icon.png' ! -name 'icon.ico' \
  ! -name 'tray' ! -name 'dots' -exec rm -rf {} +

mkdir -p src-tauri/icons/tray src-tauri/icons/dots ui/img
for pair in ok:#22C55E warn:#F59E0B down:#EF4444 idle:#94A3B8; do
  name="${pair%%:*}"; color="${pair#*:}"
  sed "s/STATUS_COLOR/$color/" assets/tray-template.svg > "$TMP/tray-$name.svg"
  rsvg-convert -w 64 -h 64 "$TMP/tray-$name.svg" -o "src-tauri/icons/tray/$name.png"
done
for pair in green:#22C55E amber:#F59E0B red:#EF4444 grey:#94A3B8; do
  name="${pair%%:*}"; color="${pair#*:}"
  printf '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><circle cx="16" cy="16" r="11" fill="%s"/></svg>' "$color" > "$TMP/dot-$name.svg"
  rsvg-convert -w 32 -h 32 "$TMP/dot-$name.svg" -o "src-tauri/icons/dots/$name.png"
done
cp assets/icon.svg ui/img/icon.svg
echo "icons regenerated"
