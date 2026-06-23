# Phase32 Follow-up Phase Split

Date: 2026-06-23 JST

Status: recovery output / follow-up required

## Purpose

Phase32 recovery found that the historical final sign-off roots did not cover
the current eval case set. This file splits the remaining blockers into
explicit follow-up work without claiming migration completion.

## Follow-up Phases

| phase | owner layer | blocker family | source evidence | closure proof |
| --- | --- | --- | --- | --- |
| Phase33 | eval/report recheck projection | Focused deterministic fixtures collapse specialized terminal states into generic verifier failure or generic `failed`. | `focused_worklist.md` groups: explicit-stop, evidence/completion, missing-deliverable, attempt/lifecycle, verifier-specific terminal mismatch. | `python3 tests/test_eval_report.py`; focused recheck on `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236` has zero focused assertion failures caused by report projection. |
| Phase34 | raw diagnostic classification | Current sign-off still reports raw `rc:1` / `rc_1` / unknown-contract findings, including rows whose structured fields exist but are not admitted by sign-off. | `recovery_task_ledger.md` P32-R007; non-duplicated current sign-off output; focused and large recheck summaries. | Current broad sign-off reports no `raw_undiagnostic_rc`; every nonzero row has diagnostic code, owner layer, active job, selected action, and accepted row-level disposition. |
| Phase35 | setup/profile/dev-server/readiness contract connection | Closed by Phase35. Setup/profile/dev-server rows now use current recheck authority and deterministic boundary proof where appropriate. | `focused-nextjs-dependency-setup`, `focused-nextjs-dev-server-port-conflict`, `focused-nextjs-endpoint-smoke`, `focused-nextjs-manifest-repair`, `focused-nextjs-route-integration`, `focused-phase26-setup-node-readiness`, `focused-setup-manifest-invalid`. | `phase_35/implementation_report.md`; current focused recheck reports `passed_recheck: 82`; current broad sign-off returns `status: pass`. |
| Phase36 | large real-LLM blocker ownership | Closed by Phase36. Six current large cases still fail as user tasks, but each has owner/action/target/evidence and a row disposition. | `phase_36/implementation_report.md`; `phase_36/large_row_ledger.md`; `eval/runs/current-all-local-llm/large/20260623T204816/recheck_summary.tsv`. | Large recheck reports `large_disposition=closed_owned_failure` for all six rows; broad sign-off has no unowned or contradictory large failure. |
| Phase37 | row-to-case proof reconciliation | Closed by Phase37. Adopted C rows and excluded rows now have explicit proof or rationale, and current eval cases are mapped or supplemental. | `phase_37/row_case_proof_matrix.md`; `phase_37/proof_gap_ledger.md`; `phase_37/implementation_report.md`. | C01-C54 are represented, all 91 current cases are mapped or supplemental, no adopted row closes on omitted historical evidence only, and no open `proof_gap` remains. |
| Phase38 | sign-off root admission gate | Phase32 previously accepted a smaller root bundle, and a later check accidentally duplicated the focused root as `focused-fixture`. | Current eval roots; sign-off command invocation; `eval/README.md`. | A deterministic gate verifies root labels are non-duplicated, required families are present, and the admitted roots cover the current eval case set before final sign-off is interpreted. |
| Phase39 | final closure retry | Phase32 final decision remains open until current proof roots are green and row-level proof is complete. | Phase33-Phase38 outputs. | Current broad sign-off exits zero and final decision report can truthfully declare completion or explicit non-completion. |

## Large Case Sub-ledger Requirement

Phase36 must not treat the six large cases as one generic blocker. It must
create a row for each case:

| case | current diagnostic | minimum closure fields |
| --- | --- | --- |
| `large-fastapi-app-modify` | `unknown_verifier_failure` | owner, action, target, verifier command, source excerpt or accepted limitation |
| `large-fastapi-app-new` | `tool_args_missing_required_field` | tool protocol owner, failed tool, missing field, correction action, target |
| `large-nextjs-app-modify` | `edit_target_not_found` | stale/missing edit target cause, admitted replacement target, action |
| `large-nextjs-app-new` | `read_only_step_mutation` | step-policy owner, explicit stop or admitted repair action, target admission rationale |
| `large-rust-app-modify` | `edit_target_not_found` | stale/missing edit target cause, admitted replacement target, action |
| `large-rust-app-new` | `blocked_bash_command_policy` with admitted `src/main.rs` target | large failure remains failed, but raw diagnostic and missing-target blockers are closed by Phase34 evidence projection |

Phase36 satisfied this requirement in `phase_36/large_row_ledger.md`.
All six rows are `closed_owned_failure`; no row is `accepted_external_limitation`.

## Row-to-case Proof Reconciliation Result

Phase37 satisfied the row-to-case proof reconciliation requirement in
`phase_37/row_case_proof_matrix.md`.

| gate | result |
| --- | --- |
| C01-C54 represented | pass |
| C01-C45 adopted rows have current or accepted proof | pass |
| C46-C54 excluded rows have rationale | pass |
| current 91 cases mapped or supplemental | pass |
| open proof gaps | 0 |

Phase38 still owns sign-off root admission. Phase39 still owns final closure
retry/reporting.

## Lessons Applied

- Current eval roots are the authority for completion; historical roots are
  regression evidence only.
- Successful current cases absent from historical roots are still proof gaps.
- Raw `rc:1` / `rc_1` is a closure blocker until it is classified or accepted
  with owner/action/evidence.
- Broad sign-off must not be interpreted from duplicated root labels.
- Large cases must be closed case-by-case because their failure modes differ.
- A phase may close only after it records owner layer, target, action, proof
  command, and rerun/sign-off condition.
- Follow-up work must not weaken assertions, add hidden retries, or classify
  implementation-quality failures as external limitations without evidence.

## Non-goals

- Do not weaken current focused assertions to recover a green sign-off.
- Do not mark large implementation-quality failures as external limitations
  without owner/action/evidence.
- Do not add hidden retry loops or provider/model-specific behavioral policy.
- Do not rely on historical roots for cases absent from those roots.
- Do not use a duplicated eval root label as a final sign-off substitute.
- Do not group incompatible large failures under one generic "large failed"
  disposition.

## Review Notes

- This split fixes the Phase32 process defect: completion must be based on the
  current eval case set, not a smaller historical root bundle.
- Each follow-up phase has a responsible layer and proof command family before
  runtime changes start.
- The split intentionally adds raw diagnostic and sign-off root admission as
  first-class phases because both were process gaps, not just runtime failures.
