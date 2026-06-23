# Loadmap2 Phase33 Plan

Date: 2026-06-23 JST

Status: implemented / reviewed

## Scope

Phase33 closes the eval/report recheck projection portion of the Phase32
recovery ledger.

| source | Phase33 responsibility |
| --- | --- |
| `phase_32/followup_phase_split.md` Phase33 | Focused deterministic fixtures collapse specialized terminal states into generic verifier failure or generic `failed`. |
| `phase_32/recovery_task_ledger.md` P32-R006 | Focused assertion failures are classified; Phase33 owns the eval/report projection subset. |
| `phase_32/focused_worklist.md` | Phase33 owns explicit-stop, evidence/completion, missing-deliverable, attempt/lifecycle, and verifier-specific terminal projection groups. |

Phase33 is an eval/report layer phase. It must not change runtime behavior,
minimal-loop behavior, provider transports, profile contracts, setup policy, or
large real-LLM behavior.

## Problem Statement

The current focused recheck root is:

```text
eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236
```

After the fixture-field recheck repair from Phase32, focused recheck still has
35 `failed_recheck` assertions. Inspection of representative deterministic
fixtures shows that many expected fields already exist in `meta.json` and/or
`fixture_fields`, but recheck projection can still collapse them into generic
derived values such as:

- `terminal_state=verifier_command_failed` instead of `explicit_stop`;
- `completion_evidence_status=failed` instead of `missing` or `stale`;
- `evidence_binding_status=bound` instead of `failed`;
- `attempt_outcome=failed` instead of `duplicate`, `no_progress`, or
  `explicit_stop`;
- generic verifier/setup terminal states instead of the specialized terminal
  states expected by focused cases.

The problem is not that the model failed during Phase33. Phase33 starts from
recorded eval artifacts and fixes deterministic recheck projection so report
recheck preserves already-recorded structured evidence.

## Selected Focused Groups

| group | representative cases | Phase33 expectation |
| --- | --- | --- |
| explicit-stop projection mismatch | `focused-artifact-role-scope-ownership`, `focused-out-of-scope-target-rejection`, `phase27-target-priority-tie-stop`, `phase27-verifier-command-policy`, `phase27-verifier-orchestration-safe-stop`, `phase28-ambiguous-authority-safe-stop`, `phase28-phase27-no-progress-handoff` | Recheck preserves explicit-stop terminal and owner/action fields when deterministic evidence says explicit stop. |
| evidence/completion status mismatch | `focused-completion-evidence-producers`, `focused-deliverable-obligation-freshness`, `focused-evidence-binding-failure`, `focused-evidence-binding-producers`, `focused-missing-evidence`, `phase27-artifact-completion-job` | Recheck preserves completion/evidence binding states instead of deriving generic `failed`. |
| missing-deliverable vs safe-stop mismatch | `focused-contract-conflict-explicit-stop`, `focused-generated-test-weakening-rejection`, `focused-missing-artifact-completion`, `focused-no-progress-target-switch`, `focused-tool-protocol-missing-write-path` | Recheck distinguishes eval-success missing deliverables from recovery-task safe-stop expectations. |
| attempt/lifecycle mismatch | `focused-docs-literal-mismatch`, `focused-phase26-safe-stop-evidence-binding`, `phase27-attempt-ledger-outcomes`, `phase27-repair-lifecycle-rerun` | Recheck preserves attempt outcome and lifecycle semantics from structured fixture/meta fields. |
| verifier-specific terminal mismatch | `focused-python-fastapi-assertion-mismatch`, `focused-stale-edit-target`, `phase27-focused-edit-stale-rejection`, `phase27-no-progress-deferral`, `phase27-patch-validation-rollback` | Recheck preserves specialized verifier/setup/step-policy terminal states when already recorded. |

`focused-phase26-setup-node-readiness`, `focused-setup-manifest-invalid`, and
the broader setup/profile/dev-server/readiness mismatch are Phase35-owned
unless their failure is proven to be purely recheck projection.

## Architecture Approach

Phase33 should add a narrow recheck projection boundary:

```text
meta.json + fixture_fields + failure evidence
  -> deterministic recheck projection
  -> normalize observation without discarding explicit structured fields
  -> focused expected-field assertions
```

The intended implementation shape is a small helper in the eval/report layer,
not scattered case-specific exceptions. The helper should define source
precedence for recheck fields:

1. deterministic `fixture_fields`;
2. explicit top-level `meta.json` fields emitted by eval;
3. failure evidence parsed from run output;
4. derived defaults from reason/rc.

Derived defaults must not overwrite an explicit deterministic fixture or meta
field for the same observation field.

## Horizontal Rollout

The projection fix must be generic across focused cases and profile families.
It should not special-case Next.js, Python, Rust, Gemini, Ollama, or any single
case ID.

Expected rollout:

- focused deterministic fixtures under completion, recovery-policy,
  target-verifier-patch, and contract-conflict;
- future report fixtures that use `fixture_fields`;
- existing real-LLM rows should keep their current behavior unless they already
  contain explicit structured observation fields.

Phase33 does not own:

- raw `rc:1` sign-off classification: Phase34;
- setup/profile/dev-server/readiness contract connection: Phase35;
- large real-LLM blocker ownership: Phase36;
- row-to-case current proof mapping: Phase37;
- sign-off root admission: Phase38.

## Documentation Updates

Implementation should update:

- `docs/evaluation.md` if the recheck projection contract becomes public eval
  behavior;
- `workspace/mvp/logic/anvil/loadmap2/phase_33/implementation_report.md` at
  closure time;
- Phase32 recovery files only if Phase33 changes the remaining blocker counts.

Do not rewrite Phase32 final decision to complete from Phase33 alone.

## Implementation Result

Phase33 implemented the eval/report recheck projection boundary in:

- `scripts/eval_report.py`;
- `scripts/eval_runtime_job_report.py`;
- `tests/test_eval_report.py`;
- `docs/evaluation.md`.

The focused recheck root remains:

```text
eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236
```

Measured result after Phase33:

| metric | before Phase33 | after Phase33 |
| --- | ---: | ---: |
| focused cases | 82 | 82 |
| focused successes | 9 | 9 |
| `passed_recheck` assertions | 47 | 78 |
| `failed_recheck` assertions | 35 | 4 |

Closed Phase33-owned groups:

- explicit-stop projection mismatch;
- evidence/completion status mismatch;
- missing-deliverable vs safe-stop projection mismatch;
- attempt/lifecycle projection mismatch where explicit fixture/meta evidence
  was already present;
- verifier/step-policy terminal projection mismatch where explicit fixture/meta
  evidence was already present.

Remaining focused assertion failures are not Phase33 projection failures:

| case | remaining owner |
| --- | --- |
| `focused-dispatch-manifest-repair` | Phase34/35 dispatch-action semantics; observed action is `resolve_manifest_conflict` rather than expected `add_missing_manifest_dependency`. |
| `focused-nextjs-dependency-setup` | Phase35 setup/profile/readiness connection. |
| `focused-nextjs-endpoint-smoke` | Phase35 dev-server/profile readiness connection. |
| `focused-nextjs-route-integration` | Phase35 profile/route integration and step-policy connection. |

Phase33 therefore closes the eval/report projection blocker but does not make a
final migration-complete claim.

## Exit Gate

Phase33 is complete only when:

- `python3 tests/test_eval_report.py` passes;
- `python3 -m py_compile scripts/eval_report.py scripts/eval_failure_observation.py scripts/eval_case_schema.py` passes;
- focused recheck on
  `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236`
  has zero focused assertion failures caused by eval/report projection;
- any remaining focused assertion failure is explicitly assigned to Phase34,
  Phase35, or another later phase with owner layer and proof command;
- no focused assertion is weakened or deleted to make the result green;
- current broad sign-off is not required to pass in Phase33 because raw
  diagnostic, setup/profile, large, row-coverage, and sign-off-root admission
  are later phases.

## Plan Review Result

Review findings incorporated:

- Kept Phase33 scoped to eval/report projection rather than raw diagnostic
  classification or setup/profile recovery.
- Added explicit field source precedence to avoid repeating the Phase32 issue
  where available structured fields were not honored by recheck.
- Added successful-current-case and historical-root lessons indirectly by
  preventing Phase33 from claiming final closure.
- Added a split-forward rule for failures that are not caused by recheck
  projection, avoiding another over-broad "complete" claim.
