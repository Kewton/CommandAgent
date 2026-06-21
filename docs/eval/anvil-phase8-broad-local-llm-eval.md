# Phase 8 Broad Local LLM Eval

Date: 2026-06-21

## Scope

Phase 8 evaluates the legacy-derived control stack migration across broad local
LLM runs. The goal is to measure pass/fail results and, more importantly,
whether failures are actionable by terminal state, contract layer, active job,
recovery owner, target/action evidence, and focused assertions.

Primary case sets:

- `eval/cases/smoke`
- `eval/cases/focused/control-recovery`
- `eval/cases/large`

## Baseline

Current commit at Phase 8 start: `4b13977`

Dirty flag at Phase 8 start: no

Prior broad local LLM baseline:

- Report: `docs/eval/phase11-local-llm-all-cases-20260621.md`
- Commit: `429cbbb`
- Provider/model: `ollama` / `qwen3.6:35b-a3b-coding-nvfp4`
- Smoke: 2/3
- Large: 0/6
- Total: 2/9

Phase 7 focused matrix reference:

- Report: Phase 7 focused control-recovery matrix report
- Focused dry-run root:
  `/private/tmp/commandagent-phase7-focused-dry-run/20260621T210349`
- Focused dry-run assertions: `skipped_dry_run: 16`

## Environment

Binary: `target/release/commandagent`

Primary provider/model:

- provider: `ollama`
- model: `qwen3.6:35b-a3b-coding-nvfp4`

Available local model was confirmed with:

```bash
ollama list
```

## Commands

Preflight:

```bash
cargo fmt --check
cargo test
cargo build --release
python3 tests/test_eval_report.py
scripts/check_branding.sh
```

Dry-run wiring:

```bash
scripts/eval_agent_slice.sh --dry-run --cases-dir eval/cases/smoke --out /tmp/commandagent-phase8-smoke-dry-run --runs 1 --binary target/release/commandagent
scripts/eval_agent_slice.sh --dry-run --cases-dir eval/cases/focused/control-recovery --out /tmp/commandagent-phase8-focused-dry-run --runs 1 --binary target/release/commandagent
scripts/eval_large_tasks.sh --dry-run --out /tmp/commandagent-phase8-large-dry-run --runs 1 --binary target/release/commandagent
```

Local LLM broad eval:

```bash
scripts/eval_agent_slice.sh --cases-dir eval/cases/smoke --out /tmp/commandagent-phase8-smoke-local-llm --runs 1 --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4 --binary target/release/commandagent --timeout-secs 900
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery --out /tmp/commandagent-phase8-focused-local-llm --runs 1 --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4 --binary target/release/commandagent --timeout-secs 900
scripts/eval_large_tasks.sh --runs 1 --out /tmp/commandagent-phase8-large-local-llm --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4 --binary target/release/commandagent --timeout-secs 900
```

Reports:

```bash
scripts/eval_report.py <root>
scripts/eval_report.py <root> --recheck
```

Focused reports use:

```bash
scripts/eval_report.py <focused-root> --cases-dir eval/cases/focused/control-recovery
scripts/eval_report.py <focused-root> --cases-dir eval/cases/focused/control-recovery --recheck
```

## Run Roots

Dry-run roots:

- smoke: `/private/tmp/commandagent-phase8-smoke-dry-run/20260621T212058`
- focused:
  `/private/tmp/commandagent-phase8-focused-dry-run/20260621T212058`
- large: `/private/tmp/commandagent-phase8-large-dry-run/20260621T212058`

Local LLM roots:

- smoke: `/private/tmp/commandagent-phase8-smoke-local-llm/20260621T212142`
- focused:
  `/private/tmp/commandagent-phase8-focused-local-llm/20260621T212227`
- large: `/private/tmp/commandagent-phase8-large-local-llm/20260621T212937`

## Preflight Result

Passed:

- `cargo fmt --check`
- `cargo test`: passed, 603 unit tests plus integration/doc tests
- `cargo build --release`
- `python3 tests/test_eval_report.py`: passed, 11 tests
- `scripts/check_branding.sh`

The first branding check caught an old-brand reference in this report. The
wording was corrected to legacy/control-stack terminology, and the check then
passed.

## Dry-run Result

Dry-run wiring passed for all three case sets. Reports and rechecks rendered.

| Case set | Root | Report result | Recheck result | Notes |
| --- | --- | ---: | ---: | --- |
| smoke | `/private/tmp/commandagent-phase8-smoke-dry-run/20260621T212058` | 0/3 | 0/3 | Expected dry-run missing deliverables. |
| focused control-recovery | `/private/tmp/commandagent-phase8-focused-dry-run/20260621T212058` | 0/16 | 0/16 | Focused assertions were `skipped_dry_run: 16`. |
| large | `/private/tmp/commandagent-phase8-large-dry-run/20260621T212058` | 0/6 | 6/6 | Dry-run workspaces contain placeholder expected files, so recheck can pass without model execution. |

## Smoke Result

Normal and recheck result: 2/3.

| Case | Result | Terminal state | Contract layer | Recovery job | Reason |
| --- | --- | --- | --- | --- | --- |
| `smoke-docs-readme` | pass | `ok` | `ok` | `none` | `ok` |
| `smoke-python-script` | pass | `ok` | `ok` | `none` | `ok` |
| `smoke-rust-cli` | fail | `eval_assertion_failed` | `eval_success_contract` | `source_implementation_repair` | `semantic_mismatch:src/main.rs:CommandAgent` |

Interpretation:

- Smoke remains at the prior broad baseline level, 2/3.
- The Rust smoke failure is not unknown or transport-related. It is classified
  as an eval success-contract mismatch with a source implementation repair job.

## Focused Control-recovery Result

Normal result: 10/16.

Recheck result: 10/16.

Focused assertions:

- normal: `passed: 10`, `failed: 6`
- recheck: `passed_recheck: 10`, `failed_recheck: 6`

Headline case results:

| Case | Result | Terminal state | Contract layer | Recovery job | Focused assertion |
| --- | --- | --- | --- | --- | --- |
| `focused-data-schema-completion` | pass | `ok` | `ok` | `none` | passed |
| `focused-missing-artifact-completion` | fail | `missing_deliverable` | `planning_contract` | `scaffold_materialization` | passed |
| `focused-docs-literal-mismatch` | fail | `eval_assertion_failed` | `eval_success_contract` | `source_implementation_repair` | passed |
| `focused-nextjs-dependency-setup` | pass | `ok` | `ok` | `dev_server_smoke` | failed |
| `focused-nextjs-dev-server-port-conflict` | pass | `ok` | `ok` | `dev_server_smoke` | failed |
| `focused-nextjs-endpoint-smoke` | fail | `plan_lint_failed` | `planning_contract` | `manifest_repair` | failed |
| `focused-nextjs-route-integration` | fail | `verifier_command_failed` | `verification_contract` | `source_implementation_repair` | failed |
| `focused-nextjs-tailwind-manifest-drift` | pass | `ok` | `ok` | `none` | passed |
| `focused-plan-parser-block-scalar-chomp` | pass | `ok` | `ok` | `none` | passed |
| `focused-python-import-binding` | pass | `ok` | `ok` | `none` | passed |
| `focused-python-missing-test-artifact` | pass | `ok` | `ok` | `none` | passed |
| `focused-contract-conflict-explicit-stop` | fail | `eval_assertion_failed` | `eval_success_contract` | `source_implementation_repair` | failed |
| `focused-generated-test-weakening-rejection` | pass | `ok` | `ok` | `none` | passed |
| `focused-no-progress-target-switch` | fail | `eval_assertion_failed` | `eval_success_contract` | `source_implementation_repair` | failed |
| `focused-rust-cargo-verifier-binding` | pass | `ok` | `ok` | `none` | passed |
| `focused-tool-protocol-missing-write-path` | pass | `ok` | `ok` | `none` | passed |

Focused assertion failures:

- `focused-nextjs-dependency-setup`: expected `active_job=none`, observed
  `dev_server_smoke`.
- `focused-nextjs-dev-server-port-conflict`: expected `active_job=none`,
  observed `dev_server_smoke`.
- `focused-nextjs-endpoint-smoke`: expected pass-side evidence, but observed
  `plan_lint_failed`, `planning_contract`, `manifest_repair`,
  `attempt_outcome=not_attempted`, and unknown evidence/completion binding.
- `focused-nextjs-route-integration`: expected pass-side evidence, but observed
  `verifier_command_failed`, `verification_contract`,
  `source_implementation_repair`, and unknown evidence/completion binding.
- `focused-contract-conflict-explicit-stop`: expected pass-side evidence, but
  observed `eval_assertion_failed` and `source_implementation_repair`.
- `focused-no-progress-target-switch`: expected pass-side evidence, but
  observed `eval_assertion_failed` and `source_implementation_repair`.

Interpretation:

- The focused matrix is useful as a broad classification surface: no case
  collapsed to an unowned `unknown`.
- Some focused expectations are now too optimistic or point at the wrong owner
  for live execution. This is a focused assertion / recovery expectation gap,
  not evidence to weaken runtime checks.
- Next.js endpoint and route cases still need stronger Planning Contract to
  Profile Contract handoff and pass-side evidence binding.
- Recovery-policy cases still execute as source implementation repair instead
  of explicit conflict/no-progress policy outcomes.

## Large Result

Normal and recheck result: 1/6.

| Case | Result | Terminal state | Contract layer | Recovery job | Runtime job | Reason |
| --- | --- | --- | --- | --- | --- | --- |
| `large-fastapi-app-modify` | fail | `missing_deliverable` | `planning_contract` | `test_artifact_completion` | | `missing:tests/test_app.py` |
| `large-fastapi-app-new` | fail | `verifier_command_failed` | `verification_contract` | `source_implementation_repair` | | `rc:1` |
| `large-nextjs-app-modify` | fail | `profile_contract_failed` | `profile_contract` | `route_integration_repair` | | `profile_verification:nextjs_integration_artifact_missing` |
| `large-nextjs-app-new` | pass | `ok` | `ok` | `dev_server_smoke` | `dev_server_smoke` | `ok` |
| `large-rust-app-modify` | fail | `verifier_command_failed` | `verification_contract` | `source_implementation_repair` | | `rc:1` |
| `large-rust-app-new` | fail | `verifier_command_failed` | `verification_contract` | `source_implementation_repair` | | `rc:1` |

Interpretation:

- Large pass count improved from the previous broad baseline, 0/6 to 1/6.
- The new passing case is `large-nextjs-app-new`, and it records
  `dev_server_smoke` as both recovery and runtime job.
- Remaining large failures are layer-owned:
  - FastAPI modify: planning/deliverable ownership of `tests/test_app.py`.
  - FastAPI new, Rust modify, Rust new: verifier/source repair.
  - Next.js modify: profile route integration.
- The remaining verifier failures are still too broad at the diagnostic level
  (`rc:1`), which means Track F/G/H need more semantic repair and attempt
  state before broad repair can reliably converge.

## Recheck Result

Recheck did not change live local LLM headline results:

| Case set | Normal | Recheck | Interpretation |
| --- | ---: | ---: | --- |
| smoke | 2/3 | 2/3 | Stable; Rust semantic mismatch remains. |
| focused control-recovery | 10/16 | 10/16 | Stable; same six focused assertion failures. |
| large | 1/6 | 1/6 | Stable; same large-case failures. |

One dry-run-specific note: large dry-run recheck reports 6/6 because dry-run
creates expected artifacts for wiring checks. That is not a model-quality pass
and is not used as Phase 8 runtime evidence.

## Terminal State Distribution

Live local LLM totals across smoke, focused, and large:

| Terminal state | Count |
| --- | ---: |
| `ok` | 13 |
| `eval_assertion_failed` | 4 |
| `verifier_command_failed` | 4 |
| `missing_deliverable` | 2 |
| `plan_lint_failed` | 1 |
| `profile_contract_failed` | 1 |

Interpretation:

- There were no `unknown` terminal states in the live Phase 8 run.
- The broad eval now separates planning, profile, verifier, and eval success
  contract failures well enough for layer-owned follow-up.

## Contract Layer Distribution

Live local LLM totals across smoke, focused, and large:

| Contract layer | Count |
| --- | ---: |
| `ok` | 13 |
| `eval_success_contract` | 4 |
| `verification_contract` | 4 |
| `planning_contract` | 3 |
| `profile_contract` | 1 |

Interpretation:

- The dominant remaining non-ok layers are verifier and eval-success contracts.
- Planning/profile failures are visible and no longer appear as generic
  transport or unknown errors.

## Focused Assertion Summary

Focused control-recovery live run:

| Assertion status | Count |
| --- | ---: |
| `passed` | 10 |
| `failed` | 6 |

Focused control-recovery recheck:

| Assertion status | Count |
| --- | ---: |
| `passed_recheck` | 10 |
| `failed_recheck` | 6 |

Actionable gaps:

- expected active-job semantics for successful Next.js setup/dev-server cases
  need to be clarified. A successful dev-server smoke can still legitimately
  record `active_job=dev_server_smoke`.
- pass-side evidence fields for endpoint/route success are still not reliably
  produced after live execution.
- recovery-policy cases need explicit conflict/no-progress outcomes instead of
  falling back to source implementation repair.

## Comparison With Previous Baseline

Previous broad baseline:

- smoke: 2/3
- large: 0/6
- total smoke+large: 2/9
- observed layers included planning, profile, verifier, and quality failures.

Phase 8 broad local LLM:

- smoke: 2/3
- large: 1/6
- total smoke+large: 3/9
- focused control-recovery: 10/16 headline success, 10/16 focused assertions
  passed
- no live case ended with `unknown`

What improved:

- `large-nextjs-app-new` passed and recorded `dev_server_smoke` evidence.
- Broad failures are more actionable by terminal state and contract layer.
- Focused matrix exposes expected/observed mismatches rather than only
  pass/fail counts.

What did not improve enough:

- Large FastAPI and Rust verifier failures still report `rc:1` without enough
  semantic failure clustering to choose a stronger repair action.
- Large Next.js modify still fails route integration at the profile contract.
- Recovery-policy focused cases still fall through to source repair instead of
  explicit stop or strategy-switch behavior.

## Follow-up Backlog By Roadmap Track

Track D: Active job arbiter and dispatch

- Clarify whether successful dev-server/setup cases should report
  `active_job=none` or the runtime job that proved completion. Phase 8 observed
  `dev_server_smoke` for successful Next.js setup/dev-server cases.

Track F: Semantic failure report and semantic repair plan

- Add stronger semantic clustering for verifier `rc:1` failures in FastAPI and
  Rust large cases so the next repair target/action is not broad
  source-implementation repair by default.

Track G: Repair brief, action envelope, and tool policy

- Make repair briefs for route integration and verifier failures carry a
  concrete selected target, expected evidence delta, and verifier rerun
  authority that are visible in eval reports.

Track H: Repair state and no-progress policy

- Recovery-policy focused cases still do not demonstrate explicit
  contract-conflict stop or no-progress target switch under live execution.
  Add runtime-effective policy evidence before considering those rows
  eval-proven.

Track I: Setup/profile/scaffold/dev-server

- Next.js endpoint smoke and route integration need better handoff from
  Planning Contract to Profile Contract and pass-side evidence binding.
- Large Next.js modify still needs route integration repair to converge or stop
  with a more specific profile target/action.

Track J: Eval reporting and lifecycle funnel

- Recheck can currently summarize original terminal states in ways that make
  dry-run large recheck look like a runtime pass. Keep this documented, and
  consider adding a dedicated dry-run recheck note in report output.

## Review Notes

Review findings:

- The Phase 8 run stayed within the intended architecture. It added no hidden
  retry loop, no provider-specific shared behavior, and no verifier weakening.
- The local LLM requirement was satisfied with Ollama and
  `qwen3.6:35b-a3b-coding-nvfp4`.
- The broad eval is now useful as a triage surface: all live failures have a
  terminal state and contract layer.
- The remaining failures should be addressed as layer-owned implementation work,
  not prompt-only tuning.
- Coverage table stage advancement should be conservative. The broad run
  supports that some paths are eval-observable, but several focused assertions
  still fail, so only rows with focused/live evidence should be advanced in a
  later coverage update.
