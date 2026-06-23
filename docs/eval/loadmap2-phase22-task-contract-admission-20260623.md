# Loadmap2 Phase22 Task Contract Admission

Date: 2026-06-23 JST

## Scope

Phase22 closes coverage rows C01-C03 from the Anvil loadmap2 recovery plan:

- C01: Task contract core lifecycle, constraints, completion evidence
  expectations, and bounded persistence decision.
- C02: Deterministic request signals and task contract admission.
- C03: Behavior obligation projection into owner lint and eval reporting.

## Implemented Boundary

CommandAgent now records additional Task Contract data:

- `task_contract_lifecycle`
- `task_contract_request_signals`
- `task_contract_constraints`
- `task_contract_completion_evidence`
- `behavior_obligation_owners`
- `behavior_obligation_paths`

These fields are deterministic contract evidence. They are rendered into
planning facts, correction evidence, eval reports, and focused fixture
assertions. They do not execute setup, increase retry budgets, add hidden
workflow continuation, or create provider/model-specific behavior.

Task Contract persistence is intentionally bounded: contracts are visible in
plan prompts, active step facts, saved plan/evidence/session artifacts, and eval
reports. CommandAgent does not maintain a hidden cross-command task-contract
memory; later commands reconstruct the contract from public inputs and
workspace facts.

## Proof

Local commands:

| command | result |
| --- | --- |
| `cargo fmt --check` | passed |
| `cargo test task_contract` | passed |
| `cargo test plan_lint` | passed |
| `python3 tests/test_eval_report.py` | passed |
| `python3 tests/test_eval_signoff.py` | passed |
| `cargo test` | passed |
| `cargo build --release` | passed |

Focused fixture root:

```text
eval/runs/loadmap2-phase22-focused-fixtures/20260623T102658
```

Focused result:

| case | result |
| --- | --- |
| `focused-task-contract-admission` | passed, assertion passed and recheck passed |
| `focused-behavior-obligation-projection` | passed, assertion passed and recheck passed |

Broad sign-off:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase22-focused-fixtures/20260623T102658 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result: `status: pass`.

## Coverage Decision

Rows C01-C03 are promoted to `Implemented` in
`docs/eval/legacy-control-stack-coverage-20260621.md`.

No Phase22 row is split forward. Later phases still own C04-C12 and should not
reuse Phase22 completion as proof for artifact role, workspace scope,
artifact ownership, ledger, completion binding, or active-job dispatch parity.
