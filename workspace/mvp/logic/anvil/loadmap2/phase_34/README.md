# Loadmap2 Phase34 Plan

Date: 2026-06-23 JST

Status: implemented / verified

## Scope

Phase34 closes the raw diagnostic classification portion of the Phase32
recovery ledger.

| source | Phase34 responsibility |
| --- | --- |
| `phase_32/followup_phase_split.md` Phase34 | Current sign-off still reports raw `rc:1` / `rc_1` / unknown-contract findings. |
| `phase_32/recovery_task_ledger.md` P32-R007 | Raw diagnostic coverage must leave no unowned raw diagnostic in sign-off output. |
| `phase_32/focused_worklist.md` | One remaining dispatch-action semantic mismatch may be adjacent to Phase34, but setup/profile/dev-server rows remain Phase35-owned. |

Phase34 is an eval/report and sign-off admission phase. It must not change
runtime behavior, minimal-loop behavior, provider transports, profile
contracts, setup execution, or model prompts.

## Current Evidence

Current roots:

```text
smoke:   eval/runs/current-all-local-llm/smoke/20260623T203030
focused: eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236
large:   eval/runs/current-all-local-llm/large/20260623T204816
```

Current broad sign-off command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

Observed Phase34-owned blocker from current `recheck_summary.tsv`:

| family | case | reason | diagnostic | target | current sign-off finding |
| --- | --- | --- | --- | --- | --- |
| large | `large-rust-app-new` | `rc:1` | `rc_1` | blank | `raw_undiagnostic_rc`, `missing_target` |

The run has richer evidence outside the normalized row:

- stderr reports `minimal loop reached max iterations`;
- repair packet reports `initial turn` / `repair turn` turn-error evidence;
- repair packet reports `bash command blocked as Unknown: compound shell commands, pipes, redirects, and shell substitutions are blocked`;
- workspace contains `src/main.rs`, `src/lib.rs`, `src/args.rs`, and
  `src/errors.rs`;
- profile facts identify `src/main.rs|src/lib.rs` as Rust entrypoints and
  integration artifacts.

## Problem Statement

The current row is not an unclassified verifier failure in practice. It is a
bounded execution/recovery failure where the recorded evidence identifies at
least two actionable causes:

1. the initial minimal-loop turn exhausted its iteration budget;
2. the repair turn attempted a Bash command shape blocked by tool policy.

However, recheck/sign-off currently sees only:

```text
reason=rc:1
diagnostic_code=rc_1
target_path=<blank>
target_admission_status=unknown
```

That makes sign-off fail with `raw_undiagnostic_rc` and `missing_target`.
Phase34 must project the existing evidence into a deterministic diagnostic and
target admission record without changing runtime behavior or rerunning the
case.

## Architecture Approach

Phase34 should add a narrow sign-off-safe diagnostic admission boundary:

```text
run meta + stderr/stdout + repair packet + workspace/profile artifacts
  -> deterministic raw diagnostic classifier
  -> row-level diagnostic_code / source_of_truth / target candidate
  -> sign-off admission check
```

The classifier should be data-only and conservative:

- derive `minimal_loop_max_iterations` from bounded minimal-loop exhaustion;
- derive `blocked_bash_command_policy` from blocked Bash policy evidence;
- prefer tool-policy evidence over generic `rc_1` when both are present;
- admit a target only from deterministic evidence sources:
  - repair packet target fields;
  - verifier/diagnostic target fields;
  - profile entrypoint/integration artifacts for the current profile;
  - existing workspace files under the run workspace;
- leave a row blocked if no deterministic target candidate can be admitted.

Expected owner/action mapping:

| diagnostic | owner | action | target handling |
| --- | --- | --- | --- |
| `minimal_loop_max_iterations` | `execution_loop` or `source` depending on target evidence | `stop_with_structured_evidence` or `edit_source_for_diagnostic` only when target evidence exists | Do not invent target. |
| `blocked_bash_command_policy` | `tool_policy` | `replace_blocked_bash_with_allowed_command` / `stop_with_structured_evidence` | Target may be `not_applicable` only if the row is admitted as tool-policy boundary with owner/action/attempt outcome. |

For `large-rust-app-new`, the likely closure path is to classify the primary
diagnostic as `blocked_bash_command_policy` or `minimal_loop_max_iterations`
and bind the row to `src/main.rs` or an explicit tool-policy non-target
disposition only if the sign-off rules allow that boundary. The implementation
phase must prove the selected mapping with a regression test.

## Layer Boundaries

| layer | Phase34 stance |
| --- | --- |
| Provider transport | No changes. Provider/model-specific behavior is out of scope. |
| Minimal loop | No changes. Do not increase max iterations or add hidden retry. |
| Step runner / repair | No runtime changes unless implementation inventory proves a runtime evidence field is already emitted but not surfaced. |
| Eval/report | Primary implementation layer for parsing existing evidence and projecting diagnostics. |
| Sign-off | May be tightened or clarified to admit explicit tool-policy/loop-boundary dispositions, but must not weaken raw diagnostic gates. |
| Profile | No new workflow behavior. Existing profile artifact facts may be used as deterministic target hints. |

## Horizontal Rollout

The classifier should apply across profiles and case families:

- Rust: `cargo` / source artifact rows;
- Python: pytest/import rows with turn-error or command-policy evidence;
- Next.js: build/dev-server rows with turn-error or command-policy evidence;
- future large rows that stop from bounded loop exhaustion or tool-policy
  rejection.

Do not add a `large-rust-app-new` case-id branch. Case-specific evidence may be
used only in tests and documentation.

## Documentation Updates

Implementation should update:

- `docs/evaluation.md` if raw diagnostic classifier/admission semantics become
  public eval behavior;
- `eval/README.md` if broad sign-off interpretation changes;
- `workspace/mvp/logic/anvil/loadmap2/phase_34/implementation_report.md` at
  closure time;
- Phase32 recovery files only with measured counts after recheck/sign-off.

## Stability And Complexity Controls

Phase34 should remain stable because it:

- reads existing artifacts only;
- uses deterministic string/field extraction;
- does not rerun models, setup, or verifiers;
- does not add hidden retry or continuation;
- leaves unknown rows blocked when evidence is insufficient.

Complexity is controlled by putting the classifier behind one eval/report
helper plus focused tests, rather than scattering row-specific exceptions in
sign-off.

## Exit Gate

Phase34 is complete only when:

- the raw diagnostic inventory is recorded before implementation;
- `large-rust-app-new` no longer reports `raw_undiagnostic_rc`;
- every nonzero current row has a useful diagnostic code or explicit accepted
  non-implementation limitation with owner/action/evidence;
- target handling is either admitted with deterministic evidence or explicitly
  non-applicable under a documented boundary rule;
- broad sign-off no longer has Phase34-owned findings;
- remaining broad sign-off findings are assigned to Phase35+ and are not
  hidden;
- no focused assertion is weakened or deleted;
- no runtime, provider, profile, setup, or minimal-loop behavior is changed
  merely to satisfy sign-off.

## Implementation Result

Phase34 is implemented.

Code changes:

- `scripts/eval_failure_observation.py` now derives useful diagnostics from
  existing stderr/stdout/repair-packet evidence when the row would otherwise
  remain `rc_1`.
- `scripts/eval_report.py` now admits an existing profile/workspace artifact
  target during `--recheck` only for failed source/route repair contexts with
  a useful diagnostic and a file that exists inside the run workspace.
- `scripts/eval_signoff.py` behavior was not weakened. Raw `rc_1` rows without
  useful evidence still fail sign-off.

Measured current-root result:

- `large-rust-app-new` changed from `diagnostic_code=rc_1` and blank target to
  `diagnostic_code=blocked_bash_command_policy`,
  `target_path=src/main.rs`, and `target_admission_status=admitted`.
- Current broad sign-off no longer reports Phase34-owned
  `raw_undiagnostic_rc` or large `missing_target` findings.
- Current broad sign-off still fails on focused assertion failures assigned to
  later phases; Phase34 does not hide or reclassify them.

Verification:

```text
python3 tests/test_eval_report.py
python3 tests/test_eval_signoff.py
python3 -m py_compile scripts/eval_report.py scripts/eval_failure_observation.py scripts/eval_signoff.py
python3 scripts/eval_report.py eval/runs/current-all-local-llm/large/20260623T204816 --cases-dir eval/cases/large --recheck
python3 scripts/eval_signoff.py --require-recheck --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

The final sign-off command exits nonzero because Phase35+ focused assertions
remain open. That is expected for Phase34 closure.

## Plan Review Result

Review findings incorporated:

- Separated Phase34 raw diagnostic classification from Phase35 setup/profile
  and dev-server readiness.
- Treated normal-summary focused assertion failures as sign-off interpretation
  context, not Phase34-owned runtime failures.
- Added explicit target-admission constraints so Phase34 cannot satisfy
  `missing_target` by inventing a source file.
- Kept the solution at the eval/report and sign-off admission boundary to
  respect CommandAgent's design principles.
- Added a cross-profile rollout rule to avoid a one-off Rust-only patch.
