# Issue #458 verification

- Status: `passed`

## Checks

- `cargo test --lib issue458`: `passed`
- `cargo test --lib auto_recovery`: `passed`
- `cargo test --test corpus_regression --test generality_guardrails --test protection_coverage_audit`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `rustfmt --edition 2024 --check src/planner/auto_recovery/issue458_tests.rs src/planner/auto_recovery/issue458_safety_tests.rs`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `git diff --check`: `passed`

Final focused run: 10 passed, 0 failed. The Recovery-focused run passed 66 tests
before the final readiness continuation refinement; the final focused and full
runs include that refinement. Final full-suite exit: 0, with 2,490 library tests
passed / 0 failed / 17 ignored; all integration and doc-test executables passed.
Existing ignored live/environment/PTY checks were not enabled. Corpus: 7 passed;
generality guards: 10 passed; protection audit: 2 passed. Growth baselines were
not changed.

Release version: `commandagent 0.1.0 f2253cb7+dirty 2026-09-09T21:45:10+09:00`.
This is the expected pre-commit build identity on verified predecessor #457.
No binary was published.

## Acceptance evidence

| Contract | Executed evidence |
| --- | --- |
| Production second attempt | Twelve corpus sequences use real preflight, prepare, start, snapshots, finish and adoption; only execution results/edits are injected. Execution failure, typed read-only stagnation and failed registered observation each reach a second attempt and verified promotion. |
| Rejection and control isolation | Source hash is unchanged after rejection and before every subsequent start. The second treatment contains original source and excludes rejected-only files. Latest diagnosis and original create provenance survive; treatment completed-artifact claims are discarded. |
| Finish terminal branches | Twelve cases cover failed verification, missing observer configuration, unavailable observation, inconsistent completion acceptance, changed authority, control drift, protected mutation, missing transaction components and permission-denied promotion. The no-observer case proves real preflight/start refusal and safe unstarted finish; the defensive post-NotConfigured branch is not made reachable by fake authority. |
| Original child safety | Nine cases check missing/corrupt/review-required child YAML, resume drift, external YAML, traversal targets, protected/private targets and protected symlink targets. Validation runs before rebuilding. All ordinary rejections retain control. |
| Credential aliases | Direct `.env` and `settings.ts -> .env` are refused with empty contract protected paths, independently in retained control and treatment. Existing traversal/confinement refusals remain. |
| Limits and interruption | Corpus limits 0/1/2, exact exhaustion and interruption agree with consumed counts. Existing real zero fast-path test emits no auto events; existing controller/CLI tests retain the maximum 20. Repeated normalized execution plans stop at two starts under limit 3. |
| Noisy observation cycles | A real registered Python checker fails in two production post-observations with different elapsed time and cwd. Readable reasons differ, but the controller stops with `cycle_detected` at two starts under limit 4. Latest diagnosis remains in the terminal error. |
| Changed diagnoses | Another real checker changes semantic stdout under the same command and reaches a third successful attempt. Identity controls separately change compiler code/message and profile reason at the same command/location. These identities remain distinct. |
| Readiness continuation | Production readiness classifier and continuation reconstruction retain typed compiler path/code/message in both identity and saved plan. Root-only changes compare equal; independent path/code/message changes compare unequal in both canonical plan and identity. No build/observation gate is bypassed. |
| Infrastructure classification | Eleven typed readiness cases refuse startup/spawn/port/timeout/environment/skipped/unknown failures and untyped build failures. Only typed compile errors or matching actual HTTP failure statuses qualify. A real readiness process using an explicit local npm fixture builds successfully then exits on startup; it returns `Unavailable(start_exited)`. The fixture does not establish a real Next.js build result. |
| Events and final projection | Corpus checks start → rejected → continuation prepared → start → promoted → complete ordering, exact used counts and stop reasons. Limit/cycle projections select the latest control-owned manual YAML. Successful projection has empty old Recovery path/command fields and no prior failure text. |
| Predecessor regression | Final full suite includes #456 real model-protocol/tool replays, #457 diagnostics, #448 promotion gates, existing observation safety, CLI artifacts, TUI completion and protection/confinement checks. |

## Execution notes and limits

Initial development runs caught invalid test-contract setup, the corpus manifest's
nonstandard key syntax, a Clippy boolean simplification and the direct/alias path
guard mismatch. Parent review added timing/path noise, semantic diagnostic
distinctions, startup-infrastructure classification and readable readiness compile
continuations. These issues were corrected and the final checks above rerun;
no gate, test expectation or baseline was weakened to obtain success.

The Recovery-focused and full suites ran outside the sandbox because existing
HTTP/process fixtures require loopback access. No live model evaluation was
requested or run, and no live success rate is claimed. Deterministic control-flow
and contract checks are passed; real-model repair efficacy remains **unmeasured**
for the subsequent #452 evaluation. No Python or GUI harness source was changed.

Read-only archive audit confirmed central event SHA-256
`f7a30626804db6072de3f0f332259cf40f6a36a2ab932d847e7e5d55b236fe38` at the
designated develop root. Existing evidence and other worktrees were not modified.
Both required predecessor branches are ancestors of this branch's parent,
`f2253cb7c55424ee9a98015230d65b9ef738aeec`; incorporation was fast-forward only.
