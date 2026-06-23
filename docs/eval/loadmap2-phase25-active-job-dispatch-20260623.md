# Loadmap2 Phase25 Active Job Dispatch

Date: 2026-06-23 JST

## Scope

Phase25 closes the C11-C12 Anvil migration rows:

- C11 active-job arbitration lifecycle
- C12 recovery owner/action dispatch gate

This phase adds explicit active-job lifecycle projection and makes dispatch
decisions visible to Recovery Task Contract rendering and eval reports. It does
not add hidden continuation, retry expansion, provider/model-specific behavior,
or a second execution engine.

## Runtime / Eval Changes

- Added `active_job_lifecycle` to contract evidence, orchestration evidence,
  recovery task rendering, deterministic fixture output, and eval summaries.
- Added selected, no-owner, ambiguous-tie, explicit-stop, and conflict-stop
  lifecycle states for active-job dispatch.
- Ensured recovery task rendering consumes selected dispatch facts such as
  owner, job, dispatch reason, candidate jobs, and tie-break reason.
- Added deterministic focused dispatch fixtures for setup, manifest, route,
  source, docs, evidence binding, verifier contract, tool protocol, no-owner,
  and ambiguous-tie paths.

## Proof

Focused fixture root:

```text
eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110
```

Commands:

```bash
cargo test active_job
cargo test recovery_orchestration
cargo test recovery_task
python3 tests/test_eval_report.py
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/dispatch --out eval/runs/loadmap2-phase25-focused-fixtures --runs 1 --proof-mode deterministic_fixture
python3 scripts/eval_report.py eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110 --cases-dir eval/cases/focused/control-recovery/dispatch --recheck
python3 scripts/eval_signoff.py --require-recheck --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 --root focused-fixture=eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110 --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result:

```text
focused assertions: passed: 10
recheck assertions: passed_recheck: 10
broad sign-off: pass
```

## Coverage Decision

C11 and C12 are promoted from `Partial` to `Implemented` in
`docs/eval/legacy-control-stack-coverage-20260621.md`.

Remaining recovery-depth rows are not closed by this phase. Recovery task
semantics, setup/profile mapping depth, target prioritization, verifier
orchestration, patch validation, and contract-conflict resolution remain owned
by later loadmap2 phases.
