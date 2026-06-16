#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

echo "== fmt =="
cargo fmt --check

echo "== test =="
cargo test

echo "== release build =="
cargo build --release

echo "== cli help =="
target/release/commandagent --help >/dev/null

echo "== branding =="
scripts/check_branding.sh

echo "== eval dry run =="
tmp_eval="$(mktemp -d)"
scripts/eval_agent_slice.sh --dry-run --out "$tmp_eval" --runs 1 >/dev/null
test -f "$tmp_eval"/*/summary.tsv

echo "offline smoke passed"
