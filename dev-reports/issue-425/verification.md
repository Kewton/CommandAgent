# Issue #425 Reopen Verification

- Status: `passed`

## Checks

- `cargo test recovery_contract_authority --lib`: `passed`
- `cargo test recovery_authority::tests --lib`: `passed`
- `cargo test planner::auto_recovery::tests --lib`: `passed`
- `cargo test known_profile_run_never_reinfers_profile --lib`: `passed`
- `cargo test ultra_final_acceptance_report_records_browser_probe_failure --lib`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo test --test profile_runtime_guardrails`: `passed`
- `rustfmt --edition 2024 --check src/planner/auto_recovery/issue425_tests.rs`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

These are fresh checks on the completed Issue #425 implementation on base
0296d779 plus WIP port 3d222aae, run on 2026-09-06. The final full test command
exited 0: library tests had 2,349 passes, zero failures and 16 ignored tests;
all integration and doc-test suites also passed, including all six corpus
tests. Default ignored tests were not enabled. Tests requiring local subprocess
or loopback access ran outside the filesystem/network sandbox. No live provider
probe was performed. Historical PR #427 checks are not used as current evidence.

## Diagnosed regressions

The first broad run had 2,346 library passes and two failures. Registering the
three exact profile-generated package-script assertions as additional final
commands duplicated the existing Next.js profile gates and applied transient
assertion-runner semantics at final acceptance. Those assertions now remain at
their step/profile gates. Focused tests verify that the final profile gate still
rejects invalid build, dev-port and start-port settings, and arbitrary Node or
different-port assertions are not exempted. The existing known-profile success
fixture and assertions are unchanged.

The HTTP 500 browser failure fixture lacked the mock build toolchain needed to
reach its intended failure after generated contracts gained their product build
verifier. It now installs the existing mock toolchain; all acceptance assertions
remain unchanged. No build or business gate was weakened, and temporary test
diagnostics were removed.

## Handoff and authority coverage

Focused tests cover read-only stagnation, phase execution/invariant failure,
final-acceptance repair failure/exhaustion, empty bounded repair, existing 4/1/1
handoffs, empty/pre-registered generated contracts, step verification presence
and absence, and competing step/phase candidates. Configured contracts at the
generated filename, canonical aliases, data registries, same-run refresh and
stale prior runs are covered. The corpus includes eight executable synthetic
handoff cases derived from the reopened evidence; it does not claim to replay
the original provider sessions.

## Independent local sessions and limits

Two newly created temporary sessions use real preflight, candidate binding and
plan preparation, followed by a deterministic test driver that repairs the
JavaScript source. Both run with an automatic Recovery budget of two:

| Session | Registered check before repair | Recovery starts | Check after repair | Persistence acceptance | Overall result |
| --- | --- | --- | --- | --- | --- |
| 1 | `node --check app.js` failed | 1 | Passed | Failed | Honest failure |
| 2 | `node --check app.js` failed | 1 | Passed | Failed | Honest failure |

These measurements demonstrate Recovery execution and retained business failure
in independent local sessions. They are syntax-check measurements, not a real
Next.js build or business E2E campaign. Regenerating live provider sessions is
excluded by the approved worker scope and remains unmeasured here. No external
session, historical evidence, live runtime state, remote lifecycle or CommandMate
service was changed. Issues #428 and #430 were not imported or reimplemented.
