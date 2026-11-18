#!/usr/bin/env bash
# Dev helper (host side): copy the working tree into the dev sandbox at ~/panelport.
# Usage: scripts/dev-sync.sh <sandbox-name>
set -euo pipefail
SANDBOX="${1:?sandbox name}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="${TMPDIR:-/tmp}/panelport-sync.tgz"
tar -czf "$TMP" -C "$ROOT" --exclude=./src-tauri/target --exclude=./node_modules --exclude=./.git .
command -v cygpath >/dev/null && TMP="$(cygpath -m "$TMP")"
sbx cp "$TMP" "$SANDBOX:/tmp/panelport-sync.tgz"
sbx exec "$SANDBOX" bash -lc 'mkdir -p ~/panelport && sudo chown "$(id -u):$(id -g)" /tmp/panelport-sync.tgz && tar -xzf /tmp/panelport-sync.tgz -C ~/panelport && rm /tmp/panelport-sync.tgz'
