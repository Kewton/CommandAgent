# Issue #253 implementation summary

## Outcome

Implemented workflow-circle schema v0.2 with registered draft-profile nodes,
paired node-level planner provider/model pins, an explicit circle-level draft
admission cap, closed-vocabulary fixtures, and contract/runbook documentation.
Existing v0 and v0.1 definitions keep their prior acceptance rules and omit all
new planner fields from events unless a v0.2 node explicitly declares the pair.

## Code

- Extended `src/workflow/schema.rs` with typed v0.2 decoding and optional
  `planner_model` / `planner_provider` node fields. Planner half-pairs, empty
  planner model IDs, planner keys in older versions, unknown providers,
  unknown conditions, and unregistered v0.2 profiles fail closed.
- Kept the v0/v0.1 admitted-only profile gate. v0.2 resolves profile IDs through
  the compiled or registered extension descriptor registry, allowing admitted
  and draft statuses without treating typos as drafts.
- Added `src/workflow/admission.rs`. After successful origin verification, any
  draft-containing workflow is projected as `circle_failed` with the existing
  `profile_not_admitted` reason; all-admitted workflows preserve the existing
  `circle_full` / `verify_origin` projection.
- Added `src/workflow/node_pins.rs` to apply explicit planner pins and provenance
  to child configs and to append those pins to workflow node events. Omission is
  a no-op, preserving existing config and event shape.
- Added only small orchestration/runner wiring: node requests carry the optional
  planner pair, v0.2 explicit events expose it, and verified terminal projection
  delegates to the admission leaf.

## Contracts and fixtures

- Revised `docs/workflow-circle-contract.md` for v0.2, including the draft
  terminal cap, planner pair rules, unregistered-profile rejection, and the
  mandatory condition-vocabulary extension procedure.
- Extended `docs/dev/workflow-smoke-runbook.md` with v0.2 preflight and terminal
  evidence checks.
- Added `tests/corpus/apps/issue253-workflow-v02/` with positive draft/planner and
  draft-terminal-cap workflows plus negative planner-half, old-version planner,
  old-version draft, unknown-condition, and unregistered-profile fixtures.
- Added `tests/issue253_workflow_v02.rs` for fixture parsing, v0.1 compatibility,
  and a provider-free end-to-end workflow closure proving a verified draft
  circle never emits or persists `circle_full`.

## Predecessor compatibility

The change uses the provider/model pair convention established by Issue #240
without modifying model-probe or classifier behavior. It does not touch the
summary or `src/lib.rs` surfaces changed by Issues #227 and #221, whose commits
were inspected but are not present on this branch.
