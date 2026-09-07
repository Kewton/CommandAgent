# Issue #439 verification

- Status: `passed`

## Checks

- `RUSTFLAGS='-D warnings' cargo test --lib issue439 -- --nocapture`: `passed`
- `RUSTFLAGS='-D warnings' cargo test --test protection_coverage_audit`: `passed`
- `bash scripts/ci.sh`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test --all-targets`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo test --test conformance`: `passed`
- `python3 scripts/validate_codex_skills.py --tracked-only`: `passed`
- `ruff check --isolated --select E4,E7,E9,F,I --ignore E402 scripts/codex_orchestrate.py scripts/validate_codex_skills.py tests/test_codex_orchestrate.py workspace/management/scripts`: `passed`
- `python3 -m pytest tests/test_codex_orchestrate.py -q`: `passed`
- `python3 -m unittest discover -s workspace/management/scripts -p 'test_*.py'`: `passed`
- `python3 tests/eval/test_acceptance_contract.py`: `passed`
- `python3 tests/eval/test_completion_contract_snapshots.py`: `passed`
- `python3 tests/eval/test_false_positive_regression.py`: `passed`
- `shellcheck scripts/*.sh`: `passed`
- `RUSTFLAGS='-D warnings' cargo test --doc`: `passed`
- `git diff --check`: `passed`

The final complete CI script exited 0. Its Rust commands ran with
`RUSTFLAGS="-D warnings"`. Focused coverage: 16 Issue tests. Full library suite:
2,411 passed, 16 existing ignored tests. Corpus: 7 passed; guardrails: 10 passed;
conformance: 18 passed, 1 existing ignored helper. Python orchestration: 71 passed;
management unittest suite: 179 passed. Doctests: 2 passed. No ignore or guardrail
baseline was added. The script documents its existing ignored Rust tests.

## Acceptance coverage

| Criterion | Evidence |
| --- | --- |
| 1: R0 no Node weak blocker | Real R0 source/smoke, four recorded completed phases, recorded browser pass, generated registration and handoff acceptance pass |
| 2: S1 reaches the next acceptance/promotion boundary | Recorded treatment source/browser pass, commands submitted through production StepPlan registration, generated contract refresh and Recovery handoff; structural acceptance passes without weak evidence |
| 3: product Node hook is not weak | Exact normalized hook identity tests and S1 acceptance; retained final command fails when the named file loses the attribute even if another file has it |
| 4: future tests retained | npm test and node --test remain registered before artifacts, remain identical through handoff, and classify as Test after creation |
| 5: honest failures | Log-only target plus unrelated assertion, inline printing, swallowed failure plus exit 0, shell masking, fake assertion import, and uncalled checks remain weak; real R0 smoke mutation fails with a command failure |
| 6: #435 and compatibility | Existing artifact-only tests retained; explicit configured S1 bytes unchanged; exact final-event key snapshot adds only weak_evidence_sources |

Live campaign and business promotion are orchestrator-owned work after integration
with #440. Recorded browser/build observations are replay inputs, not new live
measurements. Hook and use-client registries are retained because the existing
final profile gates cannot guarantee every identical source path and predicate;
see the positive/negative equivalence mapping in `design.md`.

## Corrections during verification

- Guardrails initially rejected test helpers counted as production and an event
  key list exceeding its existing test-file budget. Wrap helpers as test-only and
  extract the unchanged expected keys plus the new field into a corpus snapshot;
  all guardrails pass without baseline edits.
- Reproduced all four reviewer exit-0 counterexamples as incorrect `Test` results
  before fixing binding scope, computed mutation, and literal-boolean control
  flow. Two derived shadow/alias variants also reproduced and are rejected.
  The regression test runs all six scripts through the bounded-process wrapper
  and requires `Weak`; protection coverage is checked without new exemptions.
- The pre-existing scheduler unit test picked up shared files from actual rg
  results for `Independent`. The user-authorized input-isolation patch is
  byte-identical to #440's test file; focused pytest and Ruff pass. Its exact
  batch/order/width assertions and production scheduling code are preserved.
