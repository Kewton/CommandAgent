# Issue #479 worker verification

- Status: `passed`

All required worker-local implementation checks passed. Parent review,
exact-commit CI/UAT and integrated release checks are separate downstream gates.

## Checks

- `cargo test issue479 --lib`: `passed` (21 tests).
- `cargo test issue478 --lib`: `passed` (7 tests, including combined inventory).
- `cargo test issue466 --lib`: `passed`.
- `cargo test issue465 --lib`: `passed` (23 passed, 2 existing ignored controls).
- `cargo test setup_step_policy --lib`: `passed` (18 tests).
- `cargo test --test corpus_regression`: `passed` (7 tests).
- `cargo test --test generality_guardrails`: `passed` (10 tests).
- `cargo test ultra_final_acceptance_event_carries_generic_static_assurance --lib`: `passed`.
- `cargo test issue479_attached_inline_flags_share_classification_and_repairability --lib`: `passed`.
- `cargo test --test protection_coverage_audit`: `passed` (2 tests).
- `cargo fmt --all -- --check`: `passed`.
- `cargo clippy --all-targets -- -D warnings`: `passed`.
- `cargo test`: `passed` (exit 0; 2,588 library tests, all default integration suites and 2 doctests passed; 19 existing library ignores).
- `cargo build --release --bin commandagent`: `passed`.
- `target/release/commandagent --version`: `passed`.
- `shasum -a 256 target/release/commandagent`: `passed`.
- `git diff --check`: `passed`.

## Earlier failures and corrections

The intentional pre-fix red controls and individual classifier baseline are
recorded in [pre-fix-reproduction.md](pre-fix-reproduction.md) and
[pre-fix-inventory.json](pre-fix-inventory.json). They are distinct from the
implementation verification runs below.

The first full `cargo test` exited 101: 2,587 library tests passed, one failed,
19 were ignored. `ultra_final_acceptance_event_carries_generic_static_assurance`
found the added `command_diagnoses` field absent from its exact event-key corpus.
Only that key was added to the expected list; no existing field/name/meaning or
schema version changed. Its focused rerun passed.

The second full run passed all 2,588 library tests and progressed through
integration tests, including corpus and growth guards, then exited 101 at
`protection_coverage_table_is_green_for_current_tree`. The attached-inline test
called the validation-only API. It now calls the shared normalizer and asserts
that the original command survives unchanged. The focused inline test and both
protection-coverage tests passed; no allowlist or guard baseline was relaxed.

The third full run completed with exit 0, including the corrected event-key test,
protection-coverage audit, corpus, generality guards, remaining integration
suites and doctests. Full runs used the authorized outside-sandbox execution
environment for local HTTP fixtures. Final fmt and clippy reruns also exited 0.

During implementation, the original #465 Runner regression exposed overly broad
frozen-Weak stopping. The correction reuses the existing contract evidence-gate
predicate, preserving actual runtime-only repairability. The original related
store repair success and configuration-bypass rejection both pass without
changing their fixture or assertions. Initial clippy clone-to-slice suggestions
and a literal profile reference flagged in a new test were also corrected; no
guard exception was introduced.

## Release and environment

Local runtime: Node v24.1.0, Cargo 1.94.0. CI now explicitly selects the same Node
version to execute the saved TypeScript/package-mode controls. The workflow
parsed successfully and the selected Node version was checked. No Python
harness was changed.

Precommit release build output:

```text
commandagent 0.1.0 0dfc6e57+dirty 2026-09-13T23:17:39+09:00
SHA256 1569120942f670c148a8445c66843c59d918dce65f586549166e7bf8694baacf
```

This is the worker's precommit build, not a clean-head or integrated release
claim. A clean committed rebuild and version/hash readback follow the commit;
their exact output belongs in the handoff. Raw process logs remain temporary
and are not committed.

## Scope and remaining gates

Actual product classifier, Runner admission/formation, registered command
execution, Recovery preservation/confirmation and final acceptance are exercised.
The original five commands and #478's complete model/host duties are inventoried
in [command-inventory.md](command-inventory.md) and the generated JSON reports.
The saved type-only source uses its actual package context and explicit
`["default","module.exports"]` expectation. Structural evidence does not claim
interface-field validation or application business correctness.

No new test is ignored. Existing suite ignores remain enabled only under their
existing prerequisites. No full saved-app build, browser observation, live
model call, exact-commit parent UAT/CI or integration result is inferred from
these worker controls. Parent owns those downstream gates, Codex2 review and
PR/CI/UAT operations. No push or external lifecycle mutation was performed.
