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

echo "offline smoke passed"
