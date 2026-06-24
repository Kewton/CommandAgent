# Legacy Phase 1 Failure Observation

Date: 2026-06-21

## Scope

This note records Phase 1 of the legacy-control migration loadmap:
Failure Observation And Eval Funnel.

The change is observation-only. It does not add hidden continuation, increase
retry budgets, change provider behavior, or select repair actions.

## What Changed

- Added a shared eval observation normalizer:
  `scripts/eval_failure_observation.py`.
- New eval runs can record terminal observation fields in both `meta.json` and
  `summary.tsv`.
- `scripts/eval_report.py` can backfill terminal observations for older eval
  roots that only have `reason`, `failure_category`, and `contract_layer`.
- Reports now include terminal-state counts, diagnostic-code counts, and a
  simple lifecycle funnel based on terminal states.
- `EADDRINUSE` and `address already in use` are classified as
  `terminal_state=port_in_use`.

## Contract Boundary

The terminal observation answers:

- where the run stopped
- which broad failure class owns the stop
- which contract layer owns the stop
- which diagnostic code should be used for triage

It does not answer:

- which repair action should be selected
- whether setup should be run
- which target should be admitted
- whether the original ultra plan should continue

Those are later loadmap phases.

## Verification Expectations

Phase 1 verification should include:

- unit tests for terminal-state classification
- report tests for legacy-row backfill
- dry-run eval output with the new fields
- one focused eval/report check before using the fields for later recovery work
