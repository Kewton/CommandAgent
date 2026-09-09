# Issue #456 verification

- Status: `passed`

## Checks

- `cargo test --lib issue456`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `rustfmt --edition 2024 --check src/planner/auto_recovery/issue456_tests.rs src/planner/auto_recovery/issue456_nextjs_tests.rs src/planner/auto_recovery/issue456_language_tests.rs`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `git diff --check`: `passed`

Final full-suite exit: 0. The library result is 2,470 passed / 0 failed / 16 ignored; integration and doc-test executables also completed successfully. This includes all seven corpus regression tests, all ten generality guards, existing fix Recovery and #448/#449 regressions. Existing ignored live/environment tests were not enabled. The eleven new Issue #456 tests passed.

The release binary reports `commandagent 0.1.0 222feef7+dirty 2026-09-09T03:05:19+09:00`, the expected pre-commit build identity. No binary publication occurred.

## Acceptance evidence

| Condition | Verification |
| --- | --- |
| Pre-change create reproduction | Restored the exact `recovery_step_plan_binding.rs` from base `222feef77a4bfd1e98a60a4bd5744a287e645b48` for a negative control, retaining the new replay harness. The create driver test exited 101 at the `bind_generated` assertion because old binding requires fix-origin. Restored the fixed file afterward. This is a component negative control, not a rerun of the historical live campaign. |
| Canonical create provenance | A real initial create UltraPlan failure before acceptance records create in the capture. Treatment startup and generated StepPlan execution use the production driver; fix-origin remains absent for create. |
| Post-binding diagnosis and related definitions | Assert original goal, role diagnosis, API/store/types paths and generated definition locations in bound instructions and final provider requests. Execute the generated Read ranges themselves. Actual conversation tool results retain `role: string`, the optional API annotation, `assignee?: Member`, `error?: string`, and store API/return contracts. |
| Original Next.js contract | Exact archived contract and source replay completes `inspect-current-state` and starts `repair-unknown`. Control/treatment product hashes match. No successful Bash/Write/Edit, dependency/build lifecycle, or write-required escalation occurs during inspection. The replay deliberately stops at repair planner exhaustion before real build/model work. |
| Real editing and promotion gates | A separate deterministic generic-profile transaction reuses the archived API/store/types, edits the API through Edit, executes its registered annotation check, and promotes. Adding a registered `false` check makes host verification fail and candidate finish reject while control retains the original optional-role source. |
| Scope and protections | JS/TS scoped dispatch rejects mutations, Bash/build, glob/grep, unrelated diagnostic-only paths, protected paths, credential files, traversal, external paths, and escaping/protected symlinks. Valid ranged source reads remain available; the repair phase retains ordinary editing policies. |
| Other languages | Python `main.py` → `helper.py` and Rust `src/main.rs` → `src/helper.rs` cases cover both create and fix using the production driver. Their contract omits the helper and includes a non-text asset; binding succeeds, actual Read results contain the helper definition, inspect Write is rejected, and inspection completes before repair starts. Separate actual-registry calls reject outside/private Read and protected Edit. An unscoped continuation stays unscoped even with a later TS diagnostic. |
| Continuation | create/fix contexts survive fresh captures for `intent=recover`. Test both rebinding from the prior treatment and production startup from the original workspace using the captured context. Original goal/diagnostic and new failure evidence remain; changed contracts are rejected. |
| Missing/legacy provenance | Production startup matrix covers create, fix, manual recover and investigate, with/without legacy fix-origin. Missing new provenance adds no authority and does not reject old execution paths. Missing contracts/commands add no binding. Legacy fix binding and evidence validation remain active. |
| Bounds and compatibility | Long Japanese diagnostics remain complete across inspect steps while existing lint limits pass. Growth baselines, no-write thresholds, registered verification, final acceptance and promotion gates were not weakened. Event names/schemas remain compatible. |

## Provenance and execution notes

Read-only comparison verified all 14 copied source/config files and the archived Next.js contract against the approved develop-root archive. `tests/corpus/apps/issue456-create-recovery/provenance.json` records their hashes. The central archive event SHA-256 remains `f7a30626804db6072de3f0f332259cf40f6a36a2ab932d847e7e5d55b236fe38`.

Early focused runs found test-contract setup mistakes, an oversized instruction, and a single-phase initial-create test rejected by existing UltraPlan lint. Those were corrected and the final checks above rerun. The generality guard also required explicit cfg(test) wrappers for included test files and moving the origin hook into the existing authority leaf; no baseline was changed.

The language refinement initially let invalid/protected targets disable the JS/TS read set; the existing denial regression caught this and scope selection now uses validated targets. A required binary asset separately reproduced `TreatmentContractBindFailed` before textual-range generation was made tolerant of non-text assets. Both refinements are covered by the final focused suite. External reads use existing fatal path guards, so their rejection is checked separately from successful inspection reachability.

The final path audit added a symlink target pointing to `.env`. The denial test first failed at `env-alias.ts`; checking credential names on the canonical path as well as the requested path closes that alias bypass.

The initial sandboxed recovery run could not start existing HTTP fixture servers. The narrower #448 check reported `nextjs_route_observation_failed:start_exited`; the sandboxed run was stopped and repeated outside the sandbox, where all 218 then-selected Recovery tests passed. Final `cargo test` likewise ran outside the sandbox and exited 0. No test condition was weakened to address that environment restriction.

No Python harness was changed. No live model or Browser evaluation ran. Deterministic reply replay establishes control-flow, tool-result retention, editing and gate behavior; it does not establish full repair of the archived application's 33 diagnostics, business-contract correctness, or live-model repair success. Those evaluation results remain for #452.
