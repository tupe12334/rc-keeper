#!/usr/bin/env bash
# Fails when docs/reference is not what the code generates.
# Catches a hand-edited file (before regenerating) and a stale or missing committed file (after).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
ref="docs/reference"
fail() {
  echo "error: $ref is out of date. Run: cargo run -p xtask -- docs" >&2
  echo "       Do not edit files in $ref by hand. Change the code, then regenerate." >&2
  exit 1
}

# 1. Working tree must match the generator. Keep a copy, regenerate, compare.
snapshot="$(mktemp -d)"
trap 'rm -rf "$snapshot"' EXIT
cp -R "$ref" "$snapshot/ref"
cargo run --locked -q -p xtask -- docs >/dev/null
diff -r "$snapshot/ref" "$ref" >/dev/null || {
  diff -r "$snapshot/ref" "$ref" || true
  fail
}

# 2. Committed files must match the generator output too. Catches new, stale or removed files.
git diff --exit-code -- "$ref" >/dev/null || fail
if [ -n "$(git status --porcelain --untracked-files=all -- "$ref")" ]; then
  fail
fi

echo "generated docs are up to date"
