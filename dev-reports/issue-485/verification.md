# Issue #485 verification

- Status: `passed`

All required **worker local checks** passed on 2026-09-14. This status does not
claim parent CI/UAT, GUI qualification or external R0 success.

## Checks

- `cargo test --lib issue485 -- --nocapture`: `passed`
- `cargo test --lib issue448`: `passed`
- `cargo test --lib issue428`: `passed`
- `cargo test --test issue428_bash_path_tokens`: `passed`
- `cargo test --lib issue474`: `passed`
- `cargo test --lib issue475`: `passed`
- `cargo test --lib issue479`: `passed`
- `cargo test --lib issue484`: `passed`
- `cargo test --test corpus_regression --test generality_guardrails --test profile_runtime_guardrails --test conformance`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `rustfmt --edition 2024 --check src/planner/auto_recovery/issue485_tests.rs`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

Focused test counts, in the order above: #485 3, #448 6, #428 1 plus 5 integration,
#474 9, #475 8, #479 21, #484 13. Combined checks: corpus 7, generality guards 10,
profile runtime guards 2, conformance 18 passed / 1 pre-existing ignored. Full
`cargo test` exited 0; its library suite passed 2,612 tests with 19 pre-existing
ignored, and all subsequent integration and documentation suites passed.
No tests were newly ignored. Clippy checked all targets with warnings denied.

Environment: Darwin arm64, Rust/Cargo 1.94.0, Node v24.1.0. Socket/subprocess
checks ran outside the filesystem/network sandbox; no shared services were
started, stopped or restarted. Formatting checks ran locally. Raw logs were
kept only in temporary storage, not committed.

## Tested source and provenance

Fresh `git fetch origin develop` confirmed the initial clean worker HEAD and
remote parent both equal `0a8222643683c764fa5f9ac8bd3e5b2b47ef7728`, including
#484 `7125ce939122b3226e10cbf364ba277876578791` through PR #486. This is the
production baseline; the historical R0 HEAD is reference evidence only.

`tested-files-sha256.json` pins all 25 code/corpus files used for the final checks;
the hashes were compared to the worktree before commit. The Issue-scoped commit
containing this report is the handoff candidate; its exact SHA is provided in
the worker's final `DONE` response. Checks ran against the final uncommitted
code/corpus tree, with no subsequent implementation changes. Report-only writes
do not change that pinned tree.

The corpus manifest records R0 session
`01a09b8b-f70e-7aa1-84f1-40a18d62da17`, historical HEAD
`b7b65f8fd75793f2075491889c66372b02a06d14`, the central 86-line event stream's
SHA-256 `524d9f569a6dd702b89180e2f1acfff82b458f7afafaaf0d1da297f1a90f1122`,
and each selected event's original position/hash. The page is byte-identical to
the saved source, SHA-256
`013b9e9015d3da335226c7f55a34c9da4cd4523921674b25e5210a36b88e2ed8`.
Original completion contract SHA-256 is
`eb2b04647a28ab10508c21e9c1cd026bea293a1c7f95dda261f4bd5529d8c9ab`.

## Measured acceptance results

`acceptance-observations.json` contains five selected JSON records from the final
focused run, emitted by tests as `ISSUE485_RECORD`. All decisions and control
measurements come from product code; build/HTTP/interaction input responses are
scripted. They are not a compiler/browser/model capability measurement.

- Saved scaffold: registration passes and completion evaluation is reached;
  runtime acceptance lacks `implementation_artifact`; real driver emits
  VerificationInconsistency, suppression and stop at 0/2, followed by failed
  TUI/process projection. Control before/after SHA-256 is
  `cdd8e5ef0a9c556c53d223da72138f01592bfcdf1a4d9d5050d819b3d9b6ba0d`.
  `control_retained=true`, `restore_invoked=false`, `restore_succeeded=false`;
  final acceptance remains `not_checked`. No CurrentSuccess/completion event.
- Business all-evidence control: every required evidence tier is strong;
  CurrentSuccess, normal verification pass, external contract true, final
  `full_success`, release gate pass, full assurance and task complete are
  separately observed.
- Same artifact with missing interaction: preflight unavailable, verification
  pass, final/release/assurance partial, task partial; completion status is
  `complete_with_partial_release_gate`. Command status completed is recorded
  separately and is not full task qualification.
- Same artifact with required build failure: preflight retains
  `build_verifier_failed`; normal verification fails, external contract is false
  and terminal task fails. The existing release-derived `full_success` component
  remains visible with a non-applicable release gate. It does not override these
  three independently asserted failures and is never used alone as acceptance.
- Unrelated API: the API alone classifies as an implementation artifact, the
  original page still classifies as scaffold, runtime implementation obligations
  remain unfulfilled, final gate is incomplete and the task fails.

All controls independently retain unchanged control hashes and false restore
flags. Ordinary final acceptance is not conflated with preflight or external R0.

## Resolved development failures

Initial fixture construction exposed inaccessible test helpers and attempted
to rebind an already protected observation contract. Test-only leaf wiring and
reading the existing bound contract resolved them. Initial mock-server failures
were separated from acceptance failures: sandbox execution cannot bind sockets,
and simultaneous tests on saved port 60302 collided. The existing test transport
now uses ephemeral sockets while original plan/contract/scripts/observer port
60302 and `require_build=true` remain unchanged. No shared process was killed.

The original process-event expectation required the actual preceding TUI stop;
adding the real TUI emitter preserved the historical stop projection. Early
assertions also conflated a VerificationReport with final qualification. The
parent's design/partial/component-status resolutions confirmed the existing
layer distinction; tests now assert the real final fields and task projection,
while retaining the mandatory build-failure control. No product gate changed.

The first combined guard/full run failed because the new include file lacked an
explicit `cfg(test)` wrapper and was counted as production by the literal audit.
Adding that wrapper fixed the audit without changing any baseline. Focused,
corpus/guards, fmt, Clippy and the complete cargo test run passed afterward.

## Parent-owned verification

| Stage | Exact candidate/result |
| --- | --- |
| PR and CI | Not run by worker; parent must record results for the final DONE commit |
| Exact-HEAD UAT | Not run by worker; parent can rerun the focused command and compare `ISSUE485_RECORD` fields |
| Integration/release and live R0 | Not run by worker; no release identity or R0 success claimed |
| GUI mapping / #452 consumer | Parent follow-up; no worker GUI measurement |

The parent reported a separate synthetic check of the existing R0 consumer and
saved its reasoning/limits in `issue485-component-status-resolution.md`; that is
not this worker's exact-HEAD UAT. This worker does not modify that consumer,
schema, R0 gate, historical evidence or parent run records.
