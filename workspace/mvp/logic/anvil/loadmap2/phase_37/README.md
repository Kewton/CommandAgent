# Loadmap2 Phase37 Plan

Date: 2026-06-24 JST

Status: completed

## Scope

Phase37 closes the row-to-case proof reconciliation portion of the Phase32
recovery ledger.

| source | Phase37 responsibility |
| --- | --- |
| `phase_32/followup_phase_split.md` Phase37 | Adopted C rows still depend partly on historical roots that omitted current cases. |
| `phase_32/recovery_task_ledger.md` P32-R009 | Add row -> eval case -> proof root -> recheck result mapping for all C01-C54 adopted rows. |
| `phase_32/current_eval_manifest.md` | Current eval roots cover 91 cases while previous Phase32 sign-off roots covered 47. |

Phase37 must not claim final migration completion. Phase38 still owns
sign-off root admission, and Phase39 owns final closure retry/reporting.

## Current Evidence

Current roots:

```text
smoke:   eval/runs/current-all-local-llm/smoke/20260623T203030
focused: eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236
large:   eval/runs/current-all-local-llm/large/20260623T204816
```

Phase35 and Phase36 results:

```text
focused recheck: passed_recheck=82
large disposition: closed_owned_failure=6
broad sign-off after Phase36: status=pass
```

Important interpretation:

- Phase35 closed focused assertion projection for the current focused root.
- Phase36 closed large ownership/disposition accounting for the current large
  root.
- Neither result proves that every adopted coverage row has a current
  row-to-case proof mapping.

## Problem Statement

The earlier final-closure attempt trusted historical proof roots that did not
cover the current eval case set. That was a process defect, not a runtime
failure.

Phase37 must make coverage proof explicit:

```text
coverage row
  -> adopted responsibility
  -> current eval case or accepted non-case proof
  -> proof root
  -> recheck/sign-off result
  -> closure disposition
```

No adopted coverage row may be considered closed only because an older phase
report says it is closed. If a current case exists, Phase37 must bind that row
to the current root and current recheck result. If no current case exists,
Phase37 must record the accepted proof authority, such as a unit test,
focused fixture root, docs-only exclusion proof, or an explicit Phase38/39
handoff.

## Architecture Approach

Phase37 should add a deterministic proof reconciliation layer in the eval/docs
surface, not a new runtime controller.

The intended data shape is:

```text
CoverageProofRow {
  coverage_id,
  adoption_decision,
  final_status,
  owning_phase,
  proof_mode,
  eval_case_id,
  matrix_row,
  proof_root,
  recheck_status,
  signoff_family,
  proof_authority,
  disposition,
  gap_reason,
}
```

This can begin as generated or curated Markdown/TSV evidence in the Phase37
directory. If the mapping proves useful beyond Phase37, later work may promote
it into a script or reusable eval report command. Phase37 should not add
hidden runtime behavior just to produce the table.

## Layer Boundaries

| layer | Phase37 stance |
| --- | --- |
| Provider transport | No changes. Proof reconciliation is provider-independent. |
| Minimal loop | No changes. Do not add retries or hidden continuation. |
| Step runner / recovery orchestration | No changes unless a proof gap exposes a genuine missing owner/action field. |
| Eval/report | Primary layer for proof root, case, and recheck-result reconciliation. |
| Coverage/docs | Primary source for C01-C54 adoption and final row state. |
| Profiles | No new profile workflow; only consume existing profile/focused case metadata. |
| Sign-off | Phase37 may prepare root coverage inputs for Phase38 but must not change final admission semantics. |

## Required Proof Dispositions

Every adopted coverage row must receive one of these dispositions:

| disposition | meaning |
| --- | --- |
| `current_eval_proven` | A current eval case exists and its current recheck/sign-off result proves the row's expected contract behavior. |
| `unit_or_fixture_proven` | The row has no current broad eval case, but has deterministic tests or focused fixture proof accepted by the coverage table. |
| `excluded_with_rationale` | The row is intentionally excluded and has coverage-table rationale. |
| `split_forward` | The row has a proof gap owned by Phase38 or Phase39 with a named closure condition. |
| `proof_gap` | The row lacks current proof and cannot be closed in Phase37. |

`proof_gap` is not a closure state. It blocks Phase37 unless it is split
forward with owner, target, proof command, and closure condition.

## Row Families

Phase37 must reconcile at least these families:

| coverage range | expected proof source |
| --- | --- |
| C01-C03 | Phase22 tests and focused task/behavior fixtures, plus current focused cases where present. |
| C04-C06 | Phase23 artifact/scope/ownership tests and focused scope fixture, plus current focused cases where present. |
| C07-C10 | Phase24 ledger/evidence/binding/freshness tests and current focused cases. |
| C11-C12 | Phase25 dispatch tests and current dispatch focused cases. |
| C13-C20 | Phase26 recovery/setup/profile/semantic/action-envelope tests and current focused cases. |
| C21-C32 | Phase27 target/verifier/repair lifecycle tests and current focused cases. |
| C33 | Phase28 contract-conflict focused cases and tests. |
| C34-C44 | Phase29 runtime-support focused fixture, targeted tests, and current case coverage where present. |
| C45 | Provider parser proof and provider-boundary tests/docs. |
| C46-C54 | Exclusion rationale or explicit existing proof for excluded rows. |

## Horizontal Rollout

Phase37 should make proof reconciliation reusable for future eval additions:

- use stable `coverage_id` values, not row order;
- use stable eval case ids, not path glob position;
- record proof root and recheck result together;
- distinguish current eval proof from historical regression proof;
- keep broad sign-off as a regression gate, not a row closure substitute;
- leave new missing mappings visible as `proof_gap`.

This applies to Python, Next.js, Rust, focused control-recovery cases, large
LLM rows, smoke rows, and future profile families.

## Documentation Updates

Implementation should update:

- `workspace/mvp/logic/anvil/loadmap2/phase_37/row_case_proof_matrix.md`;
- `workspace/mvp/logic/anvil/loadmap2/phase_37/proof_gap_ledger.md`;
- `workspace/mvp/logic/anvil/loadmap2/phase_37/implementation_report.md`;
- `phase_32/recovery_task_ledger.md` after P32-R009 is closed or split;
- `phase_32/followup_phase_split.md` if Phase37 creates a narrower Phase38/39
  handoff;
- `docs/eval/legacy-control-stack-coverage-20260621.md` only if a coverage row
  state or proof reference is corrected;
- `docs/evaluation.md` or `eval/README.md` only if proof reconciliation becomes
  a reusable public eval workflow.

## Stability And Complexity Controls

Phase37 remains stable by:

- reconciling existing evidence before adding mechanisms;
- treating current eval roots as the proof authority for current cases;
- preserving historical roots as regression evidence only;
- avoiding runtime behavior changes unless a proof gap exposes a concrete
  missing contract field;
- avoiding provider/model-specific branches;
- not weakening focused assertions or large dispositions;
- not using broad sign-off pass as row-level closure by itself.

Complexity is controlled by one row-to-case matrix and one proof-gap ledger
rather than embedding proof logic across profile, provider, and runtime layers.

## Exit Gate

Phase37 is complete only when:

- every adopted coverage row C01-C45 has a row in the proof matrix;
- every excluded row C46-C54 has a row with exclusion rationale;
- every current eval case from `phase_32/current_eval_manifest.md` is either
  bound to a coverage row or explicitly marked as supplemental regression
  proof;
- no adopted row depends only on historical roots that omitted current cases;
- every `proof_gap` is either resolved or split forward to Phase38/39 with
  owner, proof command, and closure condition;
- P32-R009 is updated to `completed`, or remains open with exact unresolved
  rows and next phase assignment;
- broad sign-off remains a regression signal and is not described as final
  migration completion;
- no hidden retry, provider/model branch, implicit setup, or verifier weakening
  is introduced.

## Plan Review Result

Review findings incorporated:

- Separated row-level proof reconciliation from sign-off root admission so
  Phase37 does not absorb Phase38.
- Required a row for excluded C46-C54 rows to prevent "implemented rows only"
  accounting from hiding exclusion rationale.
- Added a `proof_gap` disposition that blocks closure unless split forward.
- Required current eval case binding for all cases in the current manifest,
  including successful current cases absent from historical roots.
- Kept proof reconciliation in eval/docs boundaries to avoid unstable runtime
  orchestration or extra model retries.
- Added an explicit Phase38/39 handoff path for root admission or final
  closure gaps discovered by the matrix.

## Implementation Result

Phase37 is complete.

Implemented artifacts:

- `row_case_proof_matrix.md`
- `proof_gap_ledger.md`
- `implementation_report.md`

Closure result:

| gate | result |
| --- | --- |
| C01-C54 represented | pass |
| C01-C45 adopted rows have current or accepted proof | pass |
| C46-C54 excluded rows have rationale | pass |
| 91 current eval cases mapped or supplemental | pass |
| open `proof_gap` rows | 0 |
| P32-R009 | completed by Phase37 |

Phase37 does not declare migration completion. Phase38 still owns sign-off
root admission, and Phase39 still owns final closure retry/reporting.
