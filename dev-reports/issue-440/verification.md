# Issue #440 verification

- Status: `passed`

## Checks

- `RUSTFLAGS='-D warnings' cargo test issue440 --lib`: `passed`
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

The full CI script completed with exit 0 on the final implementation. It sets
`RUSTFLAGS=-D warnings` for its Rust checks. The final focused run passed all
11 tests; doctests passed both compile-fail contracts. Existing ignored tests
remain unchanged. The Python scheduler module passed all 71 tests.

## Acceptance evidence

1. The sanitized I1 Recovery fixture preserves short successful Read batches
   between capped replies. Default two stops after source event 607 and does
   not request 609. Internal limit three stops after 629 and does not request
   631. Matching provider totals are 262217 ms and 385947 ms, respectively,
   below the unchanged 900000 ms cap. Recorded-time replay exercises duration
   accounting without sleeping; actual mock-provider loop executions separately
   assert no additional request at either boundary, including the original
   `plan-run-step` / `implement` / `inspect-current-state` scope.
2. Native and normalized text Write with the synthetic 16 KiB page, plus Edit,
   remain allowed at 8192 tokens and reset the streak. Failed Edit, Read,
   shorter replies, and missing usage do not reset it. Unknown usage and a
   zero `num_predict` do not count as measured cap evidence.
3. Early failure saves Recovery Markdown and parseable UltraPlan YAML with
   count and total matching duration. A separate auto-Recovery test captures
   the typed candidate registered by the existing plan writer.
4. Persisted `provider_turn_duration` contains actual returned-call count and
   successful Write/Edit execution facts. Tests cover both event writers,
   nested provider calls, event order, offered-tool count preservation,
   one-time flush on continue/error/unwind, scope/path/thread isolation, and
   anonymous spill above the 64 KiB in-memory tail threshold.
5. Event names and `schema_version = "1"` are preserved. Legacy events without
   the new fields produce unchanged time-profile totals. The new stop retains
   provider-time attribution without double-counting its duration.

## Resolved verification findings

- The first full Rust attempt hit sandbox-denied local HTTP listeners; it was
  interrupted and the complete script was rerun outside the sandbox. All final
  checks above passed in that environment. No live model call was made.
- A pre-existing Python unit fixture searched repository text for `Independent`
  and inferred incidental shared files. Its enrichment input is now isolated;
  the exact batching/order/parallel-limit assertions and separate enrichment
  tests are retained. Ruff and the entire module passed afterward.
- The protection audit interpreted a scoped-thread `.spawn` test expression
  as child-process launch. The test now uses explicit `std::thread::spawn`;
  the audit rules, allowlists, and growth baselines were not changed. Both
  protection-coverage tests passed within the final all-targets run.

## Scope

Base: `36f73eeb312c105a62ed488ce54e480b9e3c8855`. Verification is deterministic
local regression coverage, not a replay campaign against the live model. The
orchestrator must combine #439 and #440 before campaign revalidation. Original
campaign/reference evidence and live `.anvil/` state were not edited.
