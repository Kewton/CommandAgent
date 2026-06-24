# Loadmap2 Phase38 Plan

Date: 2026-06-24 JST

Status: completed / reviewed

## Scope

Phase38 closes the sign-off root admission gate portion of the Phase32
follow-up split.

| source | Phase38 responsibility |
| --- | --- |
| `phase_32/followup_phase_split.md` Phase38 | Phase32 previously accepted a smaller root bundle, and a later check accidentally duplicated the focused root as `focused-fixture`. |
| `phase_32/recovery_task_ledger.md` exit gate item 1 | Current eval manifest and sign-off roots must cover the same case set. |
| `phase_37/proof_gap_ledger.md` P37-H001 | Root labels, duplicate roots, required families, and current case-set coverage need a deterministic gate before final sign-off is interpreted. |

Phase38 must not declare final migration completion. Phase39 still owns final
closure retry/reporting after root admission is proven.

## Current Evidence

Current roots:

```text
smoke:   eval/runs/current-all-local-llm/smoke/20260623T203030
focused: eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236
large:   eval/runs/current-all-local-llm/large/20260623T204816
```

Current proof surface from Phase37:

| family | cases | admission expectation |
| --- | ---: | --- |
| smoke | 3 | required |
| focused/control-recovery | 82 | required as `focused` |
| large | 6 | required |
| small | 0 | not required while no YAML cases exist |

Phase37 result:

```text
C01-C54 represented
current 91 cases mapped or supplemental
open proof gaps: 0
```

## Problem Statement

Broad sign-off can only be trusted after the input root bundle is admitted.
The earlier failure mode was process-level:

```text
sign-off command accepts a root bundle
  -> labels or roots are duplicated / missing / stale
  -> sign-off result is interpreted as migration proof
  -> migration state becomes unreliable
```

Phase38 must add a deterministic root-admission gate before final sign-off is
interpreted.

The gate should validate:

- root labels are unique;
- root paths are unique unless an explicit alias policy says otherwise;
- required families are present;
- each root has the expected summary/recheck artifact;
- each root family matches the admitted label;
- current case counts match the current eval manifest or an explicit manifest
  override;
- duplicated focused roots such as `focused` plus `focused-fixture` cannot be
  used as final proof;
- a smaller historical root bundle cannot satisfy the current final sign-off.

## Architecture Approach

Phase38 should add a deterministic eval/sign-off admission contract:

```text
SignoffRootAdmission {
  expected_manifest_id,
  required_families,
  admitted_roots,
  rejected_roots,
  duplicate_labels,
  duplicate_paths,
  family_case_counts,
  current_case_coverage,
  admission_status,
  admission_reason,
}
```

The preferred implementation layer is `scripts/eval_signoff.py` plus targeted
tests. If the current script already has a suitable boundary, extend it there.
If it does not, add a small helper module or function that remains
read-only/report-only and is reused by sign-off.

The gate must fail closed: if root coverage cannot be proven, sign-off should
return a nonzero result with an admission finding before any migration
completion report can use the output.

## Layer Boundaries

| layer | Phase38 stance |
| --- | --- |
| Provider transport | No changes. Root admission is independent of provider/model. |
| Minimal loop | No changes. Do not add retries or execution behavior. |
| Step runner / recovery orchestration | No changes. This is post-run eval admission. |
| Eval/sign-off | Primary layer. Validate root labels, root paths, family coverage, and current case counts. |
| Eval/report | May provide case counts or root metadata; should not alter row outcomes. |
| Docs/eval | Document root admission semantics and final sign-off prerequisites. |
| Phase39 final report | Consumes Phase38 admission result, but Phase38 does not write the final migration decision. |

## Admission Contract

The Phase38 gate should admit a final-current sign-off only when all are true:

| check | rule |
| --- | --- |
| label uniqueness | No duplicate `--root <label>=...` labels. |
| path uniqueness | No same root path under multiple labels for final-current sign-off. |
| required families | `smoke`, `focused`, and `large` are present for the current manifest. |
| optional families | `small` is optional only while the current manifest records zero small cases. |
| family identity | Root contents match the expected family label and case id shape. |
| summary availability | `summary.tsv` or `recheck_summary.tsv` exists as required by `--require-recheck`. |
| case coverage | Admitted roots cover 91 current cases: 3 smoke, 82 focused, 6 large. |
| historical-root rejection | Historical root bundles cannot satisfy final-current sign-off when they omit current cases. |
| admission evidence | The sign-off output records admitted and rejected roots with reasons. |

## Horizontal Rollout

Phase38 should generalize admission by family manifest rather than hard-coding
one command:

- future eval families can add an expected case count and required/optional
  flag;
- root identity is derived from root metadata, case ids, and root path, not
  model/provider details;
- current-case coverage is checked by stable case ids and family labels;
- duplicated supplemental roots can still be allowed for regression reports
  outside final-current sign-off if explicitly marked non-final.

This avoids a brittle one-off fix for the Phase32 duplicated focused-root
incident.

## Documentation Updates

Implementation should update:

- `eval/README.md` with final-current sign-off root admission requirements;
- `docs/evaluation.md` if root admission becomes public evaluation behavior;
- `workspace/mvp/logic/anvil/loadmap2/phase_38/root_admission_report.md` at
  closure time;
- `workspace/mvp/logic/anvil/loadmap2/phase_38/implementation_report.md` at
  closure time;
- `phase_32/recovery_task_ledger.md` after exit gate item 1 is satisfied;
- `phase_32/followup_phase_split.md` after Phase38 is closed.

## Stability And Complexity Controls

Phase38 remains stable by:

- checking existing files and case ids instead of rerunning models;
- failing closed when root coverage cannot be proven;
- emitting explicit admission findings instead of silently accepting roots;
- avoiding provider/model-specific behavior;
- avoiding hidden retry, implicit setup, or verifier weakening;
- keeping final migration decision out of the admission gate.

Complexity is controlled by one admission contract and one sign-off integration
point rather than spreading root validation across report generation,
profiles, or runtime code.

## Exit Gate

Phase38 is complete only when:

- duplicate labels are rejected with a deterministic finding;
- duplicate root paths under different labels are rejected for final-current
  sign-off;
- missing required families are rejected;
- historical roots that omit current cases cannot satisfy final-current
  admission;
- the current root bundle is admitted with 3 smoke, 82 focused, and 6 large
  cases;
- sign-off still reports `status: pass` for the admitted current roots;
- admission findings are visible in sign-off output or an admission report;
- docs explain root admission before final migration completion;
- Phase39 receives the admitted-current-root proof;
- no hidden retry, provider/model branch, implicit setup, or verifier
  weakening is introduced.

## Implementation Result

Phase38 completed the sign-off root admission gate in
`scripts/eval_signoff.py`.

Closure evidence:

- `root_admission_report.md`
- `implementation_report.md`
- `python3 tests/test_eval_signoff.py`
- `python3 -m py_compile scripts/eval_signoff.py`
- current positive sign-off with 91/91 admitted cases
- negative duplicated focused-root sign-off failing with `duplicate_root_path`

Phase38 does not declare migration completion. Phase39 remains responsible for
the final closure retry/reporting step.

## Plan Review Result

Review findings incorporated:

- Kept Phase38 focused on root admission rather than final closure reporting.
- Required both duplicate-label and duplicate-path rejection because the
  historical issue involved label misuse and accidental root duplication.
- Added family identity and case-count checks so a smaller historical bundle
  cannot be interpreted as current proof.
- Made `small` optional only while the current manifest has zero small cases,
  preserving future extensibility.
- Required admission evidence in output/reporting so failures remain
  observable.
- Kept the implementation in eval/sign-off boundaries to avoid unstable
  runtime orchestration.
