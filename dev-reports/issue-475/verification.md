# Issue #475 worker verification

- Status: `passed`

## Checks

- `git fetch origin develop`: `passed`
- `git rev-parse HEAD origin/develop`: `passed`
- `node --check tests/corpus/apps/issue475-store-preflight/escaped-import-route.ts`: `passed`
- `cargo test --lib issue475 -- --nocapture`: `passed`
- `cargo test --lib recovery`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `rustfmt --edition 2024 --check src/planner/auto_recovery/issue475_tests.rs`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `shasum -a 256 target/release/commandagent`: `passed`
- `git diff --check`: `passed`

## Results and scope

This is the worker-local verification status. The dispatch assigns targeted
UAT orchestration, PR/CI, merge and the integrated release gate to the parent.
This status does not mark those later gates, real browser/Next.js app UAT or
live model capability evaluation complete.

The clean worker branch began at verified predecessor #474,
`593649601397034563f6da1d2007d7a30d8dbdf3`. Fetched origin/develop remained
`e43b2d86760a77b5250afbb260106ea21abba947`, its parent. Both the #474 committed
change and its passed verification were inspected before editing. No predecessor
merge or cherry-pick was needed.

The final focused suite passed 8 tests. Recovery regression passed 294 tests
with 2 existing ignored tests, including #429/#467 positive and refusal controls.
The final full cargo test exited 0 outside the sandbox: 2,561 library tests
passed, 19 existing library tests were ignored, all integration targets passed,
and both doctests passed. Corpus (7) and generality guardrails (10) are included.
Existing ignored integration/PTY helpers are not counted as passed; no ignored
tests were added. All-target Clippy passed without warnings.

Initial socket denial was retried outside the sandbox. Scripted observer setup
initially reported offline/dependency-unavailable; the disposable test transport
was configured explicitly before recording a successful exercised path. The
first broad run found test helpers being counted as production profile literals;
explicit cfg(test) modules fixed that placement, then focused checks, Clippy and
the entire cargo test were rerun successfully. No guardrail baseline changed.

## Recognition and observer evidence

The pre-repair real Source comparison is in the corpus baseline.json. Literal
pid/diagnostic changes alone did not produce outputs; constant rename plus the
diagnostic change produced projects.json. Final tests exercise both Source and
the product policy over complete saved store/types/routes, including existing
and missing concrete files. Protected/config/source/private/outside/traversal/
symlink, parameter/call-site mutation, recursion/depth and opaque imports refuse
grants. Parent's exact escaped importer is valid Node syntax and reproduced the
new inference defect before its guard; the fixed product policy refuses it while
the normal tasks route still keeps store in the closure. See
[escaped-import-reproduction.md](escaped-import-reproduction.md).

[controlled-observations.json](controlled-observations.json) contains three
operation/stage cases and fourteen actual preflight/control audit cases. The
real product Next.js capability observer runs the saved store with scripted
build/server/HTTP input. Build/start imports and GET / preserve bytes, GET API
initializes missing collections only, and POST changes projects. Those exact
hashes match nextjs_capabilities. Protecting projects rejects the same change;
control is retained. The interaction deliberately reports business unobserved.
This establishes the local observation contract, not Next.js compilation,
browser behavior, R0 success, a historical writer or candidate adoption.

The #467 restoration matrix is reused with the generic store. Unregistered
data/source/config changes and observation errors reject and still reach
control audit. Actual restore success, actual copy failure and refusal before
restore are distinct. Unchanged control has restore_invoked=false and no
restored-file/hash fields, irrespective of the retained legacy reason string.
Historical individual operation/writer remains unknown; its recorded stage is
nextjs_capabilities. No new JSON condition was appended to the original R0 gate.

## Identity and preservation

The 21 final product/test/corpus files are bound in
[verification-evidence.json](verification-evidence.json). The inventory was
rechecked after the final full tests. Four original source files still match
their copied provenance hashes, and original central events still hash to
`8671fd95b592b5fa9ef76266db161a5903806e0b1b9ca08d06c21ea70ea1be82`.
Original JSON, reports and live .anvil were not edited. No source/test file
changed after the final Clippy/full test run.

Local release from that verified, precommit tree:

```text
commandagent 0.1.0 59364960+dirty 2026-09-13T02:59:36+09:00
SHA256 14a8d617b014ca1ae9a2bf85e50535d8e5321eeab339809ffd3645a970c1742f
```

The dirty marker and embedded timestamp are precommit source metadata, not an
integrated release identity or build completion time. This binary was not
installed into a shared launcher or runtime. Rust/cargo 1.94.0 and Node v24.1.0
were used.

## Parent handoff

Public Actions API results for the exact fetched origin/develop parent SHA are
recorded in [parent-ci.json](parent-ci.json): CI, acceptance and Next.js domain
oracles were all completed/success. These are parent results, not CI for this
worker commit. The parent must run this commit's assigned UAT/CI, merge when
appropriate, and verify the integrated release version/hash. The independent
#474 diagnostic follow-up remains outside this Issue.

Codex2 completed two read-only discussions, both exit 0 / source history / DONE,
recorded in [discussion.md](discussion.md). No push, PR, merge, Issue lifecycle
mutation, model campaign, original evidence rewrite or runtime migration was
performed by this worker.
