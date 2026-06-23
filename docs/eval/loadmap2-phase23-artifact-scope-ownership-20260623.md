# Loadmap2 Phase23 Artifact Scope Ownership

Date: 2026-06-23 JST

## Scope

Phase23 closed C04-C06:

- C04 artifact role taxonomy
- C05 task workspace scope
- C06 artifact ownership

This phase strengthened shared contract data only. It did not add a hidden
workspace crawler, retry loop, provider/model branch, or new active-job
dispatch path.

## Implementation Summary

- Added a shared `ArtifactKind` to `ArtifactRole` projection boundary.
- Added `raw_input` and `derived_output` roles so data artifacts do not collapse
  into `unknown` or source repair.
- Kept raw inputs protected and non-repairable.
- Kept derived outputs as deliverable artifacts without introducing a new
  active-job workflow.
- Exposed excluded workspace paths through `WorkspaceScope`.
- Tightened completion authority so only in-scope owned non-generated
  deliverables can satisfy required paths.
- Aligned eval role fallback with runtime raw/derived data roles.
- Added a focused deterministic fixture for role/scope/ownership assertions.

## Proof

Commands passed:

```bash
cargo fmt --check
cargo test profile_artifact
cargo test artifact_graph
cargo test workspace_scope
cargo test workspace_snapshot
cargo test artifact_ownership
cargo test target_admission
cargo test artifact_completion
cargo test evidence_authority
python3 tests/test_eval_report.py
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/planning --out eval/runs/loadmap2-phase23-focused-fixtures --runs 1 --proof-mode deterministic_fixture
python3 scripts/eval_report.py eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023 --cases-dir eval/cases/focused/control-recovery/planning --recheck
cargo test
cargo build --release
python3 scripts/eval_signoff.py --require-recheck --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 --root focused-fixture=eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023 --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result:

```text
focused fixture root: eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023
focused assertions: passed_recheck
broad sign-off: pass
```

## Coverage Decision

| row | decision |
| --- | --- |
| C04 | Implemented |
| C05 | Implemented |
| C06 | Implemented |

No Phase23 row is split forward. Remaining artifact-ledger producer breadth,
completion/evidence-binding producers, richer workspace candidate discovery,
and repair-lifecycle behavior belong to later coverage rows.
