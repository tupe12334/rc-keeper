#!/usr/bin/env bash
# Fails when the generated API files are not what the code generates.
# Covers docs/reference and ui/src/api/generated.ts.
# Catches a hand-edited file (before regenerating) and a stale or missing committed file (after).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
generated=(docs/reference ui/src/api/generated.ts)
fail() {
  echo "error: generated API files are out of date. Run: cargo run -p xtask -- docs" >&2
  echo "       Do not edit these files by hand: ${generated[*]}" >&2
  exit 1
}

# 1. Working tree must match the generator. Keep a copy, regenerate, compare.
snapshot="$(mktemp -d)"
trap 'rm -rf "$snapshot"' EXIT
cp -R docs/reference "$snapshot/reference"
cp ui/src/api/generated.ts "$snapshot/generated.ts"
cargo run --locked -q -p xtask -- docs >/dev/null
diff -r "$snapshot/reference" docs/reference >/dev/null || {
  diff -r "$snapshot/reference" docs/reference || true
  fail
}
diff "$snapshot/generated.ts" ui/src/api/generated.ts >/dev/null || {
  diff "$snapshot/generated.ts" ui/src/api/generated.ts || true
  fail
}

# 2. Committed files must match the generator output too. Catches new, stale or removed files.
git diff --exit-code -- "${generated[@]}" >/dev/null || fail
if [ -n "$(git status --porcelain --untracked-files=all -- "${generated[@]}")" ]; then
  fail
fi

echo "generated API files are up to date"
