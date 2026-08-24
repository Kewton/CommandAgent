#!/usr/bin/env bash
set -euo pipefail

export RUSTFLAGS="-D warnings"

echo "Ignored Rust tests: 32 (documented in docs/dev/ci-ignored-tests.md)"
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --test corpus_regression
cargo test --test generality_guardrails
cargo test --test conformance

python3 scripts/validate_codex_skills.py --tracked-only
ruff check --isolated --select E4,E7,E9,F,I --ignore E402 \
  scripts/codex_orchestrate.py \
  scripts/validate_codex_skills.py \
  tests/test_codex_orchestrate.py \
  workspace/management/scripts
python3 -m pytest tests/test_codex_orchestrate.py -q
python3 -m unittest discover -s workspace/management/scripts -p 'test_*.py'
python3 tests/eval/test_acceptance_contract.py
python3 tests/eval/test_completion_contract_snapshots.py
python3 tests/eval/test_false_positive_regression.py

shellcheck scripts/*.sh
