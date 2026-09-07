# Issue #420 reopened verification

- Status: `passed`

## Checks

- `cargo test --lib issue420`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

Final verification completed on 2026-09-07. Full `cargo test` exited 0 outside
the sandbox so local HTTP fixtures could bind their sockets. All default library,
integration, and doc-test suites passed; default ignored tests were not enabled.
This includes corpus_regression, generality_guardrails, profile_runtime_guardrails,
conformance, issue420_scaffold_contract, the prior `session_01a06793` API-contract
regression, existing short-circuit tests, and predecessor #435/#425 behavior.
No Python harness changed, so Ruff and pytest were not required.

## Acceptance evidence

- The ten new focused tests pass. A two-step planner execution creates the API
  route, then requires the UI step to write `src/app/page.tsx`. Its terminal event
  is `completed` with that path and `changed_path_count: 1`.
- All 11 historical iteration probes are replayed against the recognized engine
  scaffold and a previously implemented API route. The old run-wide contract
  still passes, fixing the #422-only false-positive expectation, but the step
  guard refuses short-circuit. The replay produces zero completed placeholder
  steps. Read-only exhaustion, unrelated Write, and textual-final cases cannot
  escape the gate; enforce, observe, and no-contract paths are covered.
- Proven existing non-placeholder satisfaction completes without a write. Its
  iteration event has the real `implement-page` ID, `write_or_edit_seen: false`,
  empty placeholder paths, matching before/after SHA-256, and the satisfied
  completion requirements. The runner terminal event has changed count 0.
- Bash replacement completes with `page.tsx` in changed paths despite no
  successful Write/Edit observation. A failing contract cannot short-circuit.
  Recovery and synthesized-precheck mutation requirements remain enforced.
- The source-only nine-run event projection preserves the exact disjoint
  re-audit: 19 events = 8 skipped + 2 failed + 3 completed with changes +
  6 completed unchanged; 11 historical shortcut objects lack step IDs. These
  outcomes are retained separately from new replay expectations, and legitimate
  skip, failed termination, and later repair changes are not labeled no-op bugs.

## Conditions and limits

Before edits, predecessor #435's verified commit
`0384999cb3a74c2603f035cadb3d0b0faddae9bc` was inspected and fast-forwarded into
this branch. The final branch retains that parent and its evidence/authority
rules. No guardrail baseline was raised. Earlier development checks exposed
missing fixture build-script evidence, a fixture table syntax error, and wiring
size limits; the inputs and module placement were corrected, and the final full
suite passed without weakening any gate or failure expectation.

Each of the nine original event files was SHA-256 verified against its existing
result.json before fixture projection. Original line numbers and hashes are in
`tests/corpus/apps/issue420-step-completion`. S3/E3's identical page and S3's
package are copied unchanged; the API is a documented minimal representative.
The tests retain the historical build-script declaration as source evidence;
they do not claim a fresh generated-app production build or browser result.

No live nine-run campaign was run. Exact scaffold recognition does not prove
arbitrary domain correctness, and existing final acceptance/release/promotion
checks remain necessary. Historical evidence, generated originals, `.anvil`
runtime state, credentials, and data-directory authority were not modified.
No push, PR creation, or Issue lifecycle mutation was performed.
