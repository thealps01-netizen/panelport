#!/usr/bin/env bash
# Portable-paths audit: fails when a file contains a machine-specific path (a user name or
# home folder). Covers hidden and ignored files except .git, node_modules and build output.
set -uo pipefail
cd "$(dirname "$0")/.."
hits="$(grep -rEn -I -f scripts/machine-paths.txt . \
  --exclude-dir=.git --exclude-dir=node_modules --exclude-dir=target --exclude-dir=gen \
  --exclude=machine-paths.txt \
  | grep -vE '/home/runner' || true)"
if [ -n "$hits" ]; then
  echo "Machine-specific paths found:"
  echo "$hits"
  exit 1
fi
echo "portable-paths audit: no machine-specific paths"
