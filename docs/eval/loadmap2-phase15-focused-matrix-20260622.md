# Loadmap2 Phase 15 Focused Matrix

Date: 2026-06-22
Base commit before changes: `8332e41`

## Scope

Phase 15 completed the focused control-recovery matrix for adopted historical
control paths. The matrix now records `matrix_row` and `proof_mode` per case so
coverage is explicit and reviewable.

## Eval Roots

- Dry-run wiring:
  `eval/runs/loadmap2-phase15-dry-run/20260622T171224`
- Deterministic fixture proof:
  `eval/runs/loadmap2-phase15-fixtures/20260622T171720`

Raw eval roots are local evidence and are not committed.

## Results

- Focused control-recovery cases parsed: 27
- Matrix rows reported: 27
- Deterministic fixture rows: 16
- Real-LLM rows in matrix: 11
- Fixture assertion result: 16 passed, 0 failed
- Dry-run assertion result: 27 skipped as expected

## Notes

`proof_mode=deterministic_fixture` is used only for hard-to-force failure and
reporting paths such as malformed tool protocol, stale edit target, port
conflict, no-progress, explicit stop, setup manifest invalid, and evidence
binding failures. It is not counted as model task-quality proof.

The report now includes a `Focused Matrix` section, and the runner supports
`--proof-mode deterministic_fixture` so report assertions can be checked without
invoking a local model.
