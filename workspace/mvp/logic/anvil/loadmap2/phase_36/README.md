# Loadmap2 Phase36 Plan

Date: 2026-06-24 JST

Status: completed

## Scope

Phase36 closes the large real-LLM blocker ownership portion of the Phase32
recovery ledger.

| source | Phase36 responsibility |
| --- | --- |
| `phase_32/followup_phase_split.md` Phase36 | Six current large cases fail with mixed causes; they must not be closed as one generic large failure. |
| `phase_32/recovery_task_ledger.md` P32-R008 | Classify six large failures into migration blocker, model-quality failure, or explicit limitation. |
| `phase_32/focused_worklist.md` | Focused assertions are closed by Phase35; Phase36 should not reopen focused work. |

Phase36 must not claim final migration completion. Phase37 still owns
row-to-case proof reconciliation, Phase38 owns sign-off root admission, and
Phase39 owns final closure reporting.

## Current Evidence

Current roots:

```text
smoke:   eval/runs/current-all-local-llm/smoke/20260623T203030
focused: eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236
large:   eval/runs/current-all-local-llm/large/20260623T204816
```

Phase35 result:

```text
focused recheck: passed_recheck=82
broad sign-off: status=pass
```

The Phase35 broad sign-off pass means the current rows are attributable enough
for the existing sign-off gate. It does not mean the six large rows succeeded
or that large implementation quality is closed.

Current large recheck inventory:

| case | terminal | diagnostic | owner/action | target | current gap |
| --- | --- | --- | --- | --- | --- |
| `large-fastapi-app-modify` | `verifier_command_failed` | `unknown_verifier_failure` | `source` / `repair_source_error` | `app/main.py` | Diagnostic is still weak; needs verifier command/source excerpt or explicit model-quality disposition. |
| `large-fastapi-app-new` | `tool_protocol_failed` | `tool_args_missing_required_field` | `tool_protocol` / `correct_tool_protocol` | `app/main.py` | Tool protocol fields are classified, but failed tool/missing field details are not projected into the row. |
| `large-nextjs-app-modify` | `verifier_command_failed` | `edit_target_not_found` | `source` / `correct_tool_protocol` | `app/page.tsx` | Action/owner are internally inconsistent: source owner with tool-protocol action. |
| `large-nextjs-app-new` | `step_policy_failed` | `read_only_step_mutation` | `explicit_stop` / `stop_with_structured_evidence` | `app/page.tsx` | Explicit stop is classified, but contract-layer/action semantics need a row-level accepted disposition. |
| `large-rust-app-modify` | `verifier_command_failed` | `edit_target_not_found` | `source` / `correct_tool_protocol` | `src/main.rs` | Same owner/action inconsistency as Next.js modify. |
| `large-rust-app-new` | `verifier_command_failed` | `blocked_bash_command_policy` | `source` / `edit_source_for_diagnostic` | `src/main.rs` | Phase34 closed raw/missing-target attribution, but row still needs large disposition and verifier evidence. |

## Problem Statement

Phase36 is a large-row disposition phase, not a broad retry phase.

The current sign-off gate can pass while the large rows are still failed
because the gate checks attribution quality, not task success. Phase36 must
make that distinction explicit and close each large row with one of these
dispositions:

| disposition | meaning | allowed use |
| --- | --- | --- |
| `closed_owned_failure` | The failed row has owner/action/target/evidence and is a known runtime/model-quality failure. | Use when the row is attributable but still failed. |
| `implementation_blocker` | The row exposes a CommandAgent contract or eval projection bug that must be fixed. | Use when owner/action/target/evidence are inconsistent or missing. |
| `accepted_external_limitation` | Provider/model throughput, network, or environment makes the proof impossible in the current run. | Use only with owner/action/evidence; not for ordinary implementation quality. |
| `split_forward` | The row belongs to a later phase because it is row-proof reconciliation, root admission, or final report work. | Use only with a named later phase and closure condition. |

The phase must not hide a failed large task by weakening the verifier, adding
retry, or calling implementation quality an external limitation without proof.

## Architecture Approach

Phase36 should introduce a row-level large failure ledger:

```text
large recheck row
  -> deterministic failure family
  -> owner/action/target/evidence consistency check
  -> source excerpt or accepted not-applicable evidence
  -> row disposition
  -> broad sign-off and Phase37 handoff
```

The implementation should favor common eval/report and recovery-contract
fields:

- `terminal_state`
- `diagnostic_code`
- `active_job`
- `recovery_owner`
- `repair_action`
- `selected_action`
- `target_path`
- `target_role`
- `target_admission_status`
- `evidence_binding_status`
- `completion_evidence_status`
- `attempt_outcome`
- verifier command and source excerpt metadata when available
- tool protocol failed tool / missing field metadata when available

Do not add `large-*` case-id branches to runtime behavior. Case-specific
handling is allowed only in Phase36 ledgers, tests, and documentation.

## Layer Boundaries

| layer | Phase36 stance |
| --- | --- |
| Provider transport | No changes. Do not classify model-quality as provider failure unless transport evidence exists. |
| Minimal loop | No changes. Do not increase iterations or add hidden retry. |
| Step runner / recovery orchestration | May be adjusted only if deterministic owner/action selection is inconsistent for an observed failure family. |
| Eval/report | Primary layer for row ledger, evidence projection, source excerpt fields, and large disposition reporting. |
| Sign-off | May add a stricter large-disposition check if broad sign-off currently admits contradictory rows. It must remain report-only. |
| Profiles | May provide artifact facts for target admission; must not become workflow engines. |
| Docs/eval | Must record that attribution pass and task success are separate closure concepts. |

## Row Closure Targets

| case | Phase36 target |
| --- | --- |
| `large-fastapi-app-modify` | Replace `unknown_verifier_failure` with useful verifier diagnostic or record a `closed_owned_failure` with verifier command/source excerpt evidence. |
| `large-fastapi-app-new` | Project failed tool and missing field details for tool-protocol failure, then close as owned tool-protocol failure. |
| `large-nextjs-app-modify` | Resolve owner/action mismatch so source owner does not carry a tool-protocol action unless tool-protocol evidence is present. |
| `large-nextjs-app-new` | Record explicit-stop disposition for read-only mutation with target, action, reason, and proof that no hidden repair should run. |
| `large-rust-app-modify` | Resolve owner/action mismatch for `edit_target_not_found` and record stale/missing target evidence. |
| `large-rust-app-new` | Keep Phase34 raw diagnostic closure, add large-row disposition with verifier/tool-policy evidence and source target binding. |

## Horizontal Rollout

Phase36 should generalize by failure family, not by profile:

- verifier failures: source/verifier evidence and target consistency;
- tool-protocol failures: failed tool, missing field, correction action;
- step-policy failures: explicit stop reason and target admission;
- edit-target failures: stale/missing target handling and replacement target;
- bounded tool-policy failures: policy evidence and allowed next action.

This applies to Python, Next.js, Rust, and future large cases.

## Documentation Updates

Implementation should update:

- `docs/evaluation.md` if large row disposition or source-excerpt fields
  become public eval behavior;
- `eval/README.md` if broad sign-off gains large-disposition semantics;
- `workspace/mvp/logic/anvil/loadmap2/phase_36/implementation_report.md` at
  closure time;
- Phase32 recovery files after measured recheck/sign-off;
- Phase37 handoff notes if any row is `split_forward`.

## Stability And Complexity Controls

Phase36 remains stable by:

- using existing eval roots and recheck rows as the source of truth;
- requiring one row ledger entry per large case before code changes;
- changing common failure-family projection only when evidence is observed;
- keeping sign-off report-only;
- avoiding hidden retries, provider branches, implicit setup, and verifier
  weakening;
- leaving unresolved rows open with a named owner phase.

Complexity is controlled by a single large-disposition model and shared
failure-family helpers rather than profile-specific workflow logic.

## Exit Gate

Phase36 is complete only when:

- all six large rows have a row-level ledger entry;
- each large row has a disposition from the Phase36 disposition vocabulary;
- owner/action/target/evidence are internally consistent for every failed
  large row, or the row is explicitly split forward;
- no implementation-quality failure is mislabeled as external limitation;
- broad sign-off has no unowned large failure and no contradictory large
  owner/action evidence;
- any remaining failed large row is documented as `closed_owned_failure`,
  `implementation_blocker`, `accepted_external_limitation`, or `split_forward`;
- Phase37 receives row-to-case proof inputs for every adopted row affected by
  large cases;
- no hidden retry, provider/model branch, implicit setup, or verifier weakening
  is added.

## Implementation Result

Phase36 is complete.

Implemented artifacts:

- `large_row_ledger.md`
- `implementation_report.md`

Measured result:

| check | result |
| --- | --- |
| focused recheck | `passed_recheck: 82` |
| large row disposition | `closed_owned_failure: 6` |
| broad sign-off | `status: pass` |

Important interpretation:

- The six large tasks still fail as application-generation tasks.
- Phase36 closes only ownership/disposition accounting for migration sign-off.
- No large row is accepted as an external limitation.
- No hidden retry, provider/model branch, implicit setup, or verifier weakening
  was added.

## Plan Review Result

Review findings incorporated:

- Distinguished broad sign-off attribution pass from actual large task success.
- Required one row ledger entry per large case to avoid the earlier
  "generic large failed" closure mistake.
- Added a disposition vocabulary so Phase36 can close attribution without
  pretending failed tasks passed.
- Added owner/action consistency as a first-class check because current rows
  include source owners with tool-protocol actions.
- Restricted `accepted_external_limitation` to provider/model throughput,
  network, or environment evidence, not ordinary implementation quality.
- Added Phase37 handoff requirements so row proof reconciliation is not lost.
