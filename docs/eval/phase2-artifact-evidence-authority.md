# Phase 2 Artifact Evidence Authority

Date: 2026-06-21

## Scope

This note records the Phase 2 contract boundary for artifact ledger and
completion authority. The change is intentionally observational and
deterministic. It does not add retry budget, hidden continuation, or a new
execution engine.

## Implemented Boundary

- `ArtifactLedgerSummary` now records artifact role, lifecycle, ownership,
  source, source of truth, whether the path was changed, whether it was
  observed, whether it is required, whether verifier output mentioned it, and
  whether it is generated or cache output.
- `CompletionAuthorityResult` classifies required deliverable completion into:
  - `missing_deliverable`
  - `missing_evidence`
  - `completion_evidence_failed`
  - `evidence_binding_failed`
  - `ok`
- `/ultra-plan-run` final required artifact checks now emit the same authority
  fields in failure text while preserving the existing missing-artifact error.
- Eval summaries and reports now include `evidence_runner_status` and
  `artifact_ledger_status` so a missing artifact is not collapsed into failed
  evidence.

## Non-Goals

- No new retry loop.
- No automatic continuation after a failed phase.
- No provider/model-specific behavior.
- No profile-wide semantic quality scoring.
- No replacement for existing verifier commands.

## Remaining Work

Pass-side producers are still intentionally narrow. File-layout evidence is
available for final required artifacts, but richer profile-specific binding
producers, verifier-proven rollback, and persistent repair-job lifecycle are
left to later roadmap phases.
