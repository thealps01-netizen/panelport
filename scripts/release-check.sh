#!/usr/bin/env bash
# Release gate: fails (non-zero exit) when a dependency has a known vulnerability.
# Needs cargo-audit (cargo install cargo-audit --locked) and npm.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== cargo audit (Rust dependencies, RustSec advisory database)"
cargo audit --file src-tauri/Cargo.lock

echo "== npm audit (build tooling)"
npm audit --audit-level=low

echo "release check passed"
