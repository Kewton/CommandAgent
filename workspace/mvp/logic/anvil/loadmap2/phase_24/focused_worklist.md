# Phase24 Focused Worklist

Date: 2026-06-23 JST

Focused proof is required because Phase24 changes or validates
producer-visible ledger, completion evidence, evidence binding, deliverable
obligation, and freshness fields.

## Required Focused Cases

Use existing focused cases only if they explicitly prove the C07-C10 fields.
Otherwise add Phase24 fixtures below.

| case | path | coverage rows | expected assertion focus |
| --- | --- | --- | --- |
| `focused-artifact-ledger-producers` | `eval/cases/focused/control-recovery/completion/artifact-ledger-producers.yaml` | C07 | ledger entries, read/changed/created/verifier/setup/scaffold paths, ledger status, source-of-truth. |
| `focused-completion-evidence-producers` | `eval/cases/focused/control-recovery/completion/completion-evidence-producers.yaml` | C08 | completion evidence kind/status/source for verifier, file layout, docs, data, report, and profile facts. |
| `focused-evidence-binding-producers` | `eval/cases/focused/control-recovery/completion/evidence-binding-producers.yaml` | C09 | binding kind/status/target/expected fields and contract evidence for missing/failed binding. |
| `focused-deliverable-obligation-freshness` | `eval/cases/focused/control-recovery/completion/deliverable-obligation-freshness.yaml` | C10 | deliverable kind/path/evidence/freshness fields and stale read-only rejection. |

## Existing Candidate Cases

| candidate | possible reuse | caveat |
| --- | --- | --- |
| `completion/missing-evidence.yaml` | C08 missing evidence and authority fields | Does not prove every producer family by itself. |
| `completion/evidence-binding-failure.yaml` | C09 failed binding authority fields | May need expanded binding kind/source assertions. |
| `completion/data-schema-completion.yaml` | C08/C09 data schema success path | May need ledger/freshness assertions. |
| `nextjs/route-integration-repair.yaml` | route/import binding success path | Does not replace common C09 producer matrix. |
| `rust/cargo-verifier-binding.yaml` | verifier/test binding success path | Does not replace docs/data/report coverage. |
| `planning/behavior-obligation-projection.yaml` | C10 task-contract obligation projection | Does not prove freshness authority. |

## Recheck Rules

- Do not update focused expected fields until unit tests prove the new fields.
- Do not mark focused failures as external limitations unless owner, action,
  evidence, and the external proof limit are already explicit.
- If a focused case fails because a ledger/evidence/binding/obligation field
  is absent, map the failure back to the C07-C10 blocker that owns the field.
- If a focused case fails because a later phase owns the missing behavior,
  record a same-surface split-forward blocker with failed proof evidence.
- If existing fixtures are reused, record exactly which fields and rows they
  prove.

## Planned Focused Execution

Suggested command if completion fixtures are added:

```bash
scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery/completion \
  --out eval/runs/loadmap2-phase24-focused-fixtures \
  --runs 1 \
  --proof-mode deterministic_fixture
```

Then recheck:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase24-focused-fixtures/<root> \
  --cases-dir eval/cases/focused/control-recovery/completion \
  --recheck
```

If proof spans additional focused directories, run one root per directory and
record row-to-root mapping in `reconciliation.md`.

## Execution Result

Executed root:

```text
eval/runs/loadmap2-phase24-focused-fixtures/20260623T115617
```

Commands:

```bash
scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery/completion \
  --out eval/runs/loadmap2-phase24-focused-fixtures \
  --runs 1 \
  --proof-mode deterministic_fixture
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase24-focused-fixtures/20260623T115617 \
  --cases-dir eval/cases/focused/control-recovery/completion
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase24-focused-fixtures/20260623T115617 \
  --cases-dir eval/cases/focused/control-recovery/completion \
  --recheck
```

Result:

- focused assertions: `passed: 6`
- recheck assertions: `passed_recheck: 6`
- C07-C10 fixture rows are present and asserted.

## Review Result

Review findings applied:

- Focused proof is mandatory for closure; only fixture creation is conditional.
- Focused assertions are limited to C07-C10 producer-visible fields.
- Existing fixtures may be reused only with explicit field-to-row mapping.
- Recheck rules prevent updating expected assertions ahead of implementation
  proof.
