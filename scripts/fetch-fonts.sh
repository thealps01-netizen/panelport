#!/usr/bin/env bash
# Fetches the three UI typefaces from the google/fonts repository (SIL OFL 1.1), subsets them to
# Latin + Turkish + the punctuation the UI uses, and writes woff2 files plus licence texts into
# ui/fonts/. Run inside the dev sandbox; needs curl and fonttools (pyftsubset) with brotli.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/ui/fonts"
PYFT="${PYFTSUBSET:-pyftsubset}"
REV="${GOOGLE_FONTS_REV:-main}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$OUT"
base="https://raw.githubusercontent.com/google/fonts/$REV/ofl"
fetch() { curl -fsSL "$base/$1" -o "$TMP/$2"; }
fetch 'antonio/Antonio%5Bwght%5D.ttf' antonio.ttf
fetch 'antonio/OFL.txt' antonio-OFL.txt
fetch 'manrope/Manrope%5Bwght%5D.ttf' manrope.ttf
fetch 'manrope/OFL.txt' manrope-OFL.txt
fetch 'jetbrainsmono/JetBrainsMono%5Bwght%5D.ttf' jetbrainsmono.ttf
fetch 'jetbrainsmono/OFL.txt' jetbrainsmono-OFL.txt
# Basic Latin, Latin-1, Latin Extended-A (Turkish ğ ı İ ş), general punctuation, arrows used in copy.
UNI="U+0020-007E,U+00A0-00FF,U+0100-017F,U+2010-2027,U+2030-203A,U+2190-2193,U+2212,U+20BA"
for f in antonio manrope jetbrainsmono; do
  "$PYFT" "$TMP/$f.ttf" --unicodes="$UNI" --layout-features='kern,liga,calt,tnum,case,ccmp,locl,mark,mkmk' \
    --flavor=woff2 --output-file="$OUT/$f.woff2"
  cp "$TMP/$f-OFL.txt" "$OUT/$f-OFL.txt"
done
( cd "$TMP" && sha256sum ./*.ttf ) > "$OUT/SOURCES.txt"
sed -i "1i # Subset from github.com/google/fonts ($REV); sha256 of the original variable TTFs:" "$OUT/SOURCES.txt"
ls -l "$OUT"
