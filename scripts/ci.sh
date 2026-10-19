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

# The providerless black-box cases run the built binary through tests/cli_cases/harness.py,
# so the binary is built first. The three pytest modules (not the runner alone) also check
# that the 149-case manifest is registered without gaps or overlap. The junit report keeps
# the executed count in the log, and a skipped test fails the run instead of counting as a pass.
echo "Providerless black-box cases (pytest: CLI/config, doctor, docs/install/REPL)"
cargo build --bin commandagent
python3 -c '
import sys

sys.path.insert(0, "tests")
from cli_cases import harness

print("providerless network guard: " + harness.detect_guard().name)
' || echo "providerless network guard: unavailable (the pytest run below fails closed)"
providerless_report="$(mktemp)"
trap 'rm -f "$providerless_report"' EXIT
python3 -m pytest -q -ra --junitxml="$providerless_report" \
  tests/test_providerless_cli.py \
  tests/test_providerless_doctor.py \
  tests/test_providerless_docs_install.py
python3 -c '
import sys
import xml.etree.ElementTree as ET

cases = list(ET.parse(sys.argv[1]).getroot().iter("testcase"))
failed = [c for c in cases if c.find("failure") is not None or c.find("error") is not None]
skipped = [
    c
    for c in cases
    if any(s.get("type") != "pytest.xfail" for s in c.findall("skipped"))
]
print("providerless pytest: %d executed, %d failed, %d skipped" % (len(cases), len(failed), len(skipped)))
sys.exit(1 if not cases or failed or skipped else 0)
' "$providerless_report"

echo "Rust dependency advisories, licenses, and bans (cargo-deny)"
cargo deny check

echo "GUI production dependency audit (npm)"
npm audit --omit=dev --audit-level=high --prefix gui

python3 scripts/validate_codex_skills.py --tracked-only
ruff check --isolated --select E4,E7,E9,F,I --ignore E402 \
  scripts/codex_orchestrate.py \
  scripts/validate_codex_skills.py \
  tests/test_codex_orchestrate.py \
  tests/cli_cases/harness.py \
  tests/test_providerless_cli.py \
  tests/test_providerless_doctor.py \
  tests/test_providerless_docs_install.py \
  workspace/management/scripts
python3 -m pytest tests/test_codex_orchestrate.py -q
python3 -m unittest discover -s workspace/management/scripts -p 'test_*.py'
python3 tests/eval/test_acceptance_contract.py
python3 tests/eval/test_completion_contract_snapshots.py
python3 tests/eval/test_false_positive_regression.py

shellcheck scripts/*.sh
