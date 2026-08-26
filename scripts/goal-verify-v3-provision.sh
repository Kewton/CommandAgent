#!/usr/bin/env bash
# Provision node_modules for the two Next.js goal_verify v3 workspaces and
# vendor them as zstd tarballs outside git. Prints the sha256 to record in
# eval/goal_verify/v0/phase6-real-workspaces-v3.json at freeze.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EXECUTION_ROOT="${GOAL_VERIFY_V3_EXECUTION_ROOT:?set GOAL_VERIFY_V3_EXECUTION_ROOT}"
OUT="${GOAL_VERIFY_V3_PROVISIONED:-$EXECUTION_ROOT/provisioned}"
mkdir -p "$OUT"
export NEXT_TELEMETRY_DISABLED=1
for case_id in create-build-only-functional create-ui-copy-style-port-path; do
  ref="$ROOT/tests/fixtures/goal_verify_v3/$case_id/reference"
  if [ -f "$ref/package-lock.json" ]; then
    (cd "$ref" && npm ci --offline --include=dev --no-audit --no-fund)
  else
    (cd "$ref" && npm install --no-audit --no-fund)
  fi
  tarball="$OUT/$case_id-pwcore-1.62.1.tar.zst"
  tar --zstd -cf "$tarball" -C "$ref" node_modules
  echo "$case_id $(shasum -a 256 "$tarball" | cut -c1-64)"
done
