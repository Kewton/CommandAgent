# Phase28 Implementation Report

Date: 2026-06-23 JST

Status: completed / closed_proven

## Scope

Phase28 closed `P20-COV-004` / C33:

- contract conflict object / decision boundary;
- source-of-truth authority decision;
- repair-target-side projection;
- selected conflict action;
- ambiguous or insufficient authority safe stop;
- Phase27 C25 no-progress conflict handoff.

## Implementation

Runtime changes:

- Added `src/agent/step_runner/contract_conflict.rs`.
- Registered the module in `src/agent/step_runner/mod.rs`.
- Updated `src/agent/step_runner/recovery_orchestration.rs` so
  `contract_conflict` evidence routes to an existing bounded recovery action
  when authority is deterministic, or explicit stop when authority is
  ambiguous/insufficient.

Eval/report changes:

- Added C33 expected/report fields in `scripts/eval_case_schema.py`,
  `scripts/eval_agent_slice.sh`, and `scripts/eval_report.py`.
- Added report tests in `tests/test_eval_report.py`.
- Added focused fixtures under
  `eval/cases/focused/control-recovery/contract-conflict/`.

Documentation changes:

- Updated `docs/architecture.md`,
  `docs/adr/0002-contract-recovery.md`, `docs/evaluation.md`, and
  `eval/README.md`.
- Updated coverage and roadmap docs for C33 closure.

## Design Compliance

- No new execution engine was added.
- No hidden retry, continuation, or provider/model-specific policy was added.
- Profiles do not own the conflict decision.
- Verifier/test weakening remains disallowed.
- Ambiguous and insufficient authority explicitly stop with structured
  evidence instead of falling back to source repair.

## Focused Proof

Focused root:

```text
eval/runs/loadmap2-phase28-contract-conflict-fixtures/20260623T152521
```

Cases:

- `phase28-source-vs-generated-test`
- `phase28-source-vs-preexisting-test`
- `phase28-docs-api-vs-source`
- `phase28-weak-verifier-contract`
- `phase28-phase27-no-progress-handoff`
- `phase28-ambiguous-authority-safe-stop`

Result:

- normal focused assertions: `passed: 6`
- recheck assertions: `passed_recheck: 6`
- unknown/raw coverage defects: none

## Broad Sign-off

Command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase27-focused-fixtures/20260623T144917 \
  --root supplemental=eval/runs/loadmap2-phase28-contract-conflict-fixtures/20260623T152521 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result:

```text
status: pass
```

## Verification Summary

Completed before closure:

- `cargo test contract_conflict --lib`
- targeted eval report tests
- Phase28 focused fixture run
- Phase28 focused fixture recheck
- broad sign-off

Full local verification was run after docs/code updates as part of closure.

## Closure Decision

C33 disposition: `closed_proven`.

No `split_forward` was created for Phase28. Later phases still own their
assigned surfaces such as language/profile/tool/workspace/runtime support and
final migration sign-off.
