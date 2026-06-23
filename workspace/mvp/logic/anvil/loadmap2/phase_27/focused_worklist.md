# Phase27 Focused Worklist

Date: 2026-06-23 JST

Focused proof is required because Phase27 changes target admission, verifier
orchestration, repair lifecycle, attempt ledger, completion job, focused edit,
mechanical fallback, and patch validation behavior.

## Required Focused Cases

Use existing focused cases only if they assert C21-C32 fields directly.
Otherwise add Phase27 fixtures under:

```text
eval/cases/focused/control-recovery/target-verifier-patch/
```

| case | path | coverage rows | expected assertion focus |
| --- | --- | --- | --- |
| `target-admission-route-source-test-docs` | new fixture matrix or extensions of existing recovery-policy/nextjs/docs cases | C21 | admitted/rejected target, target role, ownership/source of truth, scope, freshness, and rejection reason across route/source/test/docs/setup/evidence-binding. |
| `target-priority-authority-tie-stop` | new fixture | C22 | target priority components, authority-based ordering, and ambiguous same-priority explicit stop. |
| `repair-lifecycle-verifier-rerun` | new fixture | C23 | lifecycle stage, verifier rerun result, safe-stop payload, and completion source after rerun. |
| `attempt-ledger-profile-outcomes` | new fixture matrix | C24 | attempt outcome, target/role/cluster, before/after signatures, changed paths, and profile family. |
| `no-progress-strategy-matrix` | new or extension of `recovery-policy/no-progress-target-switch.yaml` | C25 | target/role/cluster exhaustion, switch/stop strategy, and no retry expansion. |
| `no-progress-contract-conflict-deferral` | new fixture | C25 / Phase28 boundary | contract-conflict branch selection and Phase28/C33 deferral without source repair fallback. |
| `verifier-diagnostic-language-matrix` | new or extensions of Rust/Python/Next.js focused cases | C26 | diagnostic code, failure kind, source excerpt, observed/expected, affected cases, candidate artifacts, weak reason, unknown count. |
| `verifier-rerun-scope-safe-stop` | new fixture | C27 | original verifier command, rerun outcome event, attempt limit, evidence binding scope, and verifier safe stop. |
| `verifier-policy-generated-test-rejection` | new fixture | C28 | generated/self-referential/unsupported assertion rejection and expectation-audit evidence. |
| `artifact-completion-job-evidence` | new or extension of completion fixtures | C29 | missing deliverable versus missing/failed/stale evidence, ownership, ledger source, freshness, completion job. |
| `focused-edit-current-excerpt` | new or extension of step-policy stale edit fixture | C30 | current excerpt availability, stale target rejection, focused edit admission status. |
| `mechanical-fallback-admission` | new fixture | C31 | mechanical adapter status/action, admitted target, verifier authority, patch handoff. |
| `patch-validation-outcomes` | new fixture matrix | C32 | patch validation status/source/outcomes, rejected paths, noop/duplicate/unsafe/test-weakening rejection, rollback admission. |

## Existing Candidate Cases

| candidate | possible reuse | caveat |
| --- | --- | --- |
| `recovery-policy/out-of-scope-target-rejection.yaml` | C21 target rejection | Must assert Phase27 target admission fields, not only final reason. |
| `recovery-policy/no-progress-target-switch.yaml` | C25 no-progress switch | Must assert exhausted target/role/cluster and selected strategy. |
| `step-policy/stale-edit-target.yaml` | C30 stale focused edit | Must assert current excerpt and stale target fields. |
| `rust/compile-diagnostic-target.yaml` | C26 diagnostic and C21 source target | Must assert diagnostic assessment and target admission separately. |
| `rust/cargo-verifier-binding.yaml` | C26/C27 verifier binding | Must assert verifier scope/rerun fields. |
| `completion/missing-artifact.yaml` | C29 artifact completion | Must assert artifact completion job and evidence authority fields. |
| `completion/missing-evidence.yaml` | C29 missing evidence | Must keep missing deliverable distinct from missing evidence. |
| `completion/evidence-binding-failure.yaml` | C21/C29 evidence-binding target and completion | Must assert target admission and binding status. |
| `nextjs/route-integration-repair.yaml` | C21 route target admission | Must assert admitted route/integration target, not only profile failure. |

## Recheck Rules

- Do not update focused expected fields until unit tests prove the new
  contract fields.
- Do not mark focused failures as external limitations unless owner, action,
  evidence, and the external proof limit are explicit.
- If a focused case fails because fields are absent, map the failure back to
  the C21-C32 row that owns the field.
- If a focused case fails because Phase28 owns the behavior, record the C33
  deferral rather than weakening C25.
- If existing fixtures are reused, record exactly which fields and rows they
  prove.

## Planned Focused Execution

Suggested command:

```bash
scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery/target-verifier-patch \
  --out eval/runs/loadmap2-phase27-focused-fixtures \
  --runs 1 \
  --proof-mode deterministic_fixture
```

Then recheck:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase27-focused-fixtures/<root> \
  --cases-dir eval/cases/focused/control-recovery/target-verifier-patch \
  --recheck
```

If proof spans additional focused directories, record every root and the
specific row it proves.

## Completed Focused Execution

Focused fixture root:

```text
eval/runs/loadmap2-phase27-focused-fixtures/20260623T144917
```

Recheck command:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase27-focused-fixtures/20260623T144917 \
  --cases-dir eval/cases/focused/control-recovery/target-verifier-patch \
  --recheck
```

Result:

- `passed_recheck: 12`
- `unknown/raw failure coverage defects: none`
- rows covered: C21, C22, C23, C24, C25, C26, C27, C28, C29, C30, C31, C32

## Review Result

Review findings applied:

- Focused proof is mandatory for closure; fixture creation is conditional on
  whether existing cases already assert the row fields.
- Focused assertions are limited to C21-C32 fields and do not close Phase28
  behavior.
- Added target, verifier, lifecycle, completion, focused edit, mechanical
  fallback, and patch validation families so Phase27 cannot pass by covering
  only selected repair success.
