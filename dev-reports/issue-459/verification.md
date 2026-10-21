# Issue #459 verification

- Status: `passed`

## Checks

- `node scripts/issue459_nextjs_matrix.mjs --work-root /private/tmp/issue459-matrix-03 --output /private/tmp/issue459-matrix-03.json --playwright-module /private/tmp/claude-501/-Users-maenokota-share-work-github-kewton-MyCodeBranchDesk/05c21a81-8f2e-467b-ab0c-5497fc1e71e8/scratchpad/npmprobe/node_modules/playwright`: `passed`
- `node scripts/issue459_replay_matrix.mjs --work-root /private/tmp/issue459-full-replay-02 --node-modules /private/tmp/issue459-matrix-03/dependencies/node_modules --playwright-module /private/tmp/claude-501/-Users-maenokota-share-work-github-kewton-MyCodeBranchDesk/05c21a81-8f2e-467b-ab0c-5497fc1e71e8/scratchpad/npmprobe/node_modules/playwright`: `passed`
- `cargo test --lib recovery_observation_policy`: `passed`
- `cargo test --lib auto_recovery`: `passed`
- `cargo test --test issue459_recovery --test corpus_regression --test generality_guardrails --test protection_coverage_audit`: `passed`
- `node --check scripts/issue459_assignment_oracle.mjs`: `passed`
- `node --check scripts/issue459_nextjs_matrix.mjs`: `passed`
- `node --check scripts/issue459_replay_matrix.mjs`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `git diff --check`: `passed`

The installed matrix passed seven expected type/build/business outcomes plus
ten valid TypeScript request-contract controls. The Recovery matrix explicitly
ran the normally ignored external integration test for all nine scenarios. Output
recognition: 25 passed, including four new rename controls. Recovery regression:
67 passed. Corpus: 7 passed; growth/general guardrails: 10 passed; protection
audit: 2 passed; hash manifest: 1 passed. Final full suite exited 0 with 2,494
library tests passed, 0 failed, 17 existing ignored tests; all integration and
doc-test executables passed. The new installed integration test is ignored by
default but was explicitly exercised nine times above. Existing ignored
live/environment/PTY checks were not enabled.

Release version: `commandagent 0.1.0 008f4f5d+dirty 2026-09-09T22:17:11+09:00`.
This is the expected pre-commit build identity on the verified #458 parent.
No binary was published. Node 24.1.0, TypeScript 5.9.3, Next.js 14.2.35,
Playwright 1.58.2 and Chromium 145.0.7632.6 were used with the frozen #457 lock
SHA-256 `95d887c375a5a4d5cf093ee39119f558e61ea999bb906e5895138babae74f832`.

## Actual Recovery outcomes

| Scenario | Starts | Registered business oracle | Actual final candidate | Control |
| --- | ---: | --- | --- | --- |
| aligned-after-no-edit | 2 | not registered | rejected: unrecognized atomic output mutation after final acceptance passed | unchanged |
| aligned-with-oracle | 1 | passed | rejected: same output-policy failure after final acceptance passed | unchanged |
| missing-list | 1 | fails: no selectable member | rejected: registered verification failure | unchanged |
| wrong-field | 1 | fails: PATCH 400; prior assignment remains after reload | rejected: registered verification failure | unchanged |
| oracle-unexecuted | 1 | not executed: missing imported module, nonzero exit | rejected: required verification failure | unchanged |
| no-edit | 2 | not registered | rejected; typed no-edit failure reaches `limit_reached` | unchanged |
| provider-stop | 1 | not registered | rejected; provider failure stops `not_recoverable` | unchanged |
| promoted-after-no-edit | 2 | not registered; independent identical-source oracle passed | **promoted**, original contract; `recovery_succeeded` | equals checked candidate |
| promoted-with-oracle | 1 | passed | **promoted**, original requirements plus strict tsc/business checks; `recovery_succeeded` | equals checked candidate |

These are actual production decisions, not expected labels alone. Both promotions
emit `recovery_plan_auto_run_complete`. The aligned-after-no-edit and
promoted-after-no-edit scenarios first reject a real no-edit treatment, prepare a
control-owned continuation, inspect a fresh snapshot,
perform actual Edit calls, pass registered checks/final acceptance and reach the
production finish decision. All rejected scenarios assert source **and data**
hash equality with original control; promoted control equals the checked candidate.

`replay-results.json` contains actual decisions, success-completion events,
candidate/oracle source hashes and control hashes. `inspection-evidence.json`
independently extracts only `phase=inspect-current-state`, non-planner provider
requests for every attempt (12 inspections), with actual prompt hashes, retained
diagnostic/path markers, Read-result hashes/excerpts, and selected real verification
and observation events. The Rust assertions use these actual messages as well as
context files; Read results from other phases cannot satisfy them. Split inspect
steps receive fresh real Reads. No inspection write/build is allowed.

The seven-case independent matrix in `fixture-results.json` retains diagnostics,
strict type/build results and browser/API operations. Original/UI-only have 33
diagnostics; role-only has 32. These are not independent-defect counts. Both
aligned variants pass listing → selection → assignment → reload, reassignment,
unassignment, status/filter/deletion and corrupt-store/no-overwrite checks.
Four selected screenshots in `browser/` come from matrix-03. The raw #457 aligned
repair passes these functions yet is rejected by Recovery's output snapshot gate;
build or business success alone never guarantees adoption.

The explicit observable repair retains atomic UUID temporary writes and generation
checks. Its provable final rename destinations are only `data/projects.json` and
`data/tasks.json`. Existing lexical/path/protected/source/symlink filters and
snapshot checks still reject dynamic helpers, source deletion and extra writes.
The original contract's capabilities/evidence are retained in all cases. The five
supplemented cases use the same installed strict compiler and `node --test` oracle;
the same commands genuinely fail the negative controls. The conservative evidence
classifier, stop limits, acceptance gates and growth baselines are unchanged.

## Development failures and final reruns

Earlier runs are retained under `/private/tmp/issue459-*`. Sandbox matrix-01
reported repeated ENOTFOUND followed by npm's exit-handler message; this did not
establish an npm defect. Sandbox replay-02 failed readiness with listen EPERM on
0.0.0.0:60302; later missing-tool-call text was secondary. Fresh dependency and
HTTP/Browser runs were executed outside the sandbox and passed. No product
repair or acceptance change was made for these environment failures.

Development assertions exposed split-inspection replay bookkeeping, an incorrect
success-event spelling, missing data in control hashes, corpus key syntax, and
the test helper's profile literal. The final replay, corpus and guard runs include
those corrections. Full-replay-01 also exposed weak evidence for direct `node`
compiler/oracle commands; the same compiler and actual assertions now execute
through their documented CLI/Node test runner forms. No classifier relaxation or
successful-result injection was used. The unrecognized original atomic helper
remains an expected negative; a bounded rename recognizer plus the semantically
equivalent explicit-destination fixture establishes the positive adoption path.

The first Recovery-focused/full-suite runs were mistakenly overlapped by this
worker and interfered on #448's fixed port 60302 (`port_in_use` in the focused
run and `start_exited` in the full run). Both were rerun
sequentially outside the sandbox with no source or expectation changes. The final
passing logs are `issue459-auto-recovery-final-02.log` and
`issue459-cargo-test-final-02.log`; other final logs include
`issue459-policy-final.log`, `issue459-guards-final-04.log`,
`issue459-clippy-final-02.log` and `issue459-release-01.log`, all in `/private/tmp`.
Raw logs, dependency caches and runtime state are not committed. No Python harness
source changed, so Ruff/Python checks were not applicable.

## Provenance and #452 handoff

The immutable input manifest binds 48 source/contract/lock/reply/harness files.
Read-only re-audit of the designated develop-root archive verified all 14
original source hashes and central-event SHA-256
`f7a30626804db6072de3f0f332259cf40f6a36a2ab932d847e7e5d55b236fe38`.
Verified #456 `32f997f4`, #457 `f2253cb7` and #458 `008f4f5d` were incorporated
by fast-forward only. Existing fix/#448/#449 and predecessor tests pass in the
Recovery/full suites. Historical evidence, other worktrees and live `.anvil/`
state were not modified.

This proves deterministic integration, not improved live-model capability. The
original generated create artifact is pre-materialized; its earlier 487-event
conversation is not replayed. Initial create failure, subsequent tools, binding,
checks, Browser, finish and adoption execute for real. Replies are authored for
the fixture. The documented sample-member source does not resolve a future R0's
business specification, and an unexecuted oracle is not a functional pass.

No real-model campaign was started. #452 must separately authorize a new R0,
retain the original goal/oracle/order, decide and record its member provider,
verify binary/dependencies/Browser availability, and execute strict types, build,
member interaction/reload, original final acceptance and candidate adoption.
The original opaque atomic helper can still fail the output gate; confirm actual
source/evidence prerequisites before starting the nine comparisons and B-1.
Do not treat this corpus as campaign success or completed four-axis evaluation.
Local commit only; push/PR/merge/Issue state actions remain with the parent.
