# Phase26 Focused Worklist

Date: 2026-06-23 JST

Focused proof is required because Phase26 changes recovery-task semantics,
setup/profile facts, semantic repair context, repair brief rendering, and
action-envelope admission before bounded repair prompt rendering.

## Required Focused Cases

Use existing focused cases only if they assert C13-C20 fields directly.
Otherwise add Phase26 fixtures under:

```text
eval/cases/focused/control-recovery/recovery-task/
```

| case | path | coverage rows | expected assertion focus |
| --- | --- | --- | --- |
| `safe-stop-evidence-binding` | `eval/cases/focused/control-recovery/recovery-task/safe-stop-evidence-binding.yaml` | C13 | safe-stop owner/job/action/target/required/disallowed/rerun fields for evidence binding failure. |
| `setup-node-readiness` | `eval/cases/focused/control-recovery/recovery-task/setup-node-readiness.yaml` | C14 | setup readiness, manifest identity, command authority, setup result, stale setup, setup failure signature. |
| `setup-rust-manifest` | `eval/cases/focused/control-recovery/recovery-task/setup-rust-manifest.yaml` | C14 | Rust manifest/toolchain setup blocker and non-Node setup policy fields. |
| `setup-python-import` | `eval/cases/focused/control-recovery/recovery-task/setup-python-import.yaml` | C14 | Python dependency/import setup blocker and non-Node setup policy fields. |
| `profile-scaffold-facts` | `eval/cases/focused/control-recovery/recovery-task/profile-scaffold-facts.yaml` | C15 | common profile output, scaffold artifacts, bounded scaffold materialization, scaffold completion evidence. |
| `profile-failure-mapping` | `eval/cases/focused/control-recovery/recovery-task/profile-failure-mapping.yaml` | C16 | typed route/manifest/setup/source/scaffold/explicit-stop profile failure mappings. |
| `semantic-conflict-object` | `eval/cases/focused/control-recovery/recovery-task/semantic-conflict-object.yaml` | C17 | conflict inputs, observed/expected, affected cases, candidate artifacts, unknown diagnostic visibility. |
| `semantic-repair-cluster-exhaustion` | `eval/cases/focused/control-recovery/recovery-task/semantic-repair-cluster-exhaustion.yaml` | C18 | selected cluster, repair role, hypothesis, expected evidence delta, exhausted cluster/role/target handoff. |
| `repair-brief-rendering` | `eval/cases/focused/control-recovery/recovery-task/repair-brief-rendering.yaml` | C19 | root cause, target, constraints, allowed/disallowed actions, confidence, preservation, success check. |
| `action-envelope-admission` | `eval/cases/focused/control-recovery/recovery-task/action-envelope-admission.yaml` | C20 | action-family admission lifecycle for setup/manifest/route/source-style repair. |
| `action-envelope-rejection` | `eval/cases/focused/control-recovery/recovery-task/action-envelope-rejection.yaml` | C20 | rejected safe-stop/tool-policy lifecycle and explicit rejection evidence. |

## Existing Candidate Cases

| candidate | possible reuse | caveat |
| --- | --- | --- |
| `completion/evidence-binding-failure.yaml` | C13 safe-stop evidence binding | Must assert packet/safe-stop fields, not only evidence-binding terminal state. |
| `completion/missing-evidence.yaml` | C13 completion-authority safe stop | Must assert missing/failed/stale evidence and explicit stop payload. |
| `nextjs/dependency-setup.yaml` | C14 Node setup | Must assert setup lifecycle and command authority fields. |
| `rust/cargo-verifier-binding.yaml` | C14 Rust setup and C17 diagnostic facts | Must assert setup policy separately from verifier binding. |
| `python/import-binding.yaml` | C14 Python setup and C17 diagnostic facts | Must assert non-Node setup policy and profile mapping fields. |
| `nextjs/route-integration-repair.yaml` | C16 profile route mapping | Must assert typed profile failure mapping before dispatch. |
| `docs/docs-literal-mismatch.yaml` | C19 repair brief docs path | Must assert repair brief, not only docs owner/action. |
| `recovery-policy/no-progress-target-switch.yaml` | C18 cluster exhaustion handoff | Must not claim Phase27 no-progress strategy closure. |
| `recovery-policy/contract-conflict-explicit-stop.yaml` | C17 conflict inputs | Proves only conflict input/safe stop, not Phase28 conflict resolution. |

## Recheck Rules

- Do not update focused expected fields until unit tests prove the new
  contract fields.
- Do not mark focused failures as external limitations unless owner, action,
  evidence, and the external proof limit are already explicit.
- If a focused case fails because fields are absent, map the failure back to
  the C13-C20 row that owns the field.
- If a focused case fails because Phase27/28 owns the missing behavior, record
  a same-surface split-forward blocker with failed proof evidence.
- If existing fixtures are reused, record exactly which fields and rows they
  prove.

## Focused Execution Result

Executed command:

```bash
scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery/recovery-task \
  --out eval/runs/loadmap2-phase26-focused-fixtures \
  --runs 1 \
  --proof-mode deterministic_fixture
```

Then recheck:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase26-focused-fixtures/<root> \
  --cases-dir eval/cases/focused/control-recovery/recovery-task \
  --recheck
```

If proof spans additional focused directories, record every root and the
specific row it proves.

Result:

```text
root: eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340
focused assertions: passed_recheck: 11
proof mode: deterministic_fixture
```

## Review Result

Review findings applied:

- Focused proof is mandatory for closure; fixture creation is conditional on
  whether existing cases already assert the row fields.
- Focused assertions are limited to C13-C20 fields and do not close Phase27
  or Phase28 behavior.
- Added safe-stop, setup, profile, semantic, repair brief, and action-envelope
  families so Phase26 cannot pass by covering only selected repair success.
