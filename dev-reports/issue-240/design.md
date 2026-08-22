# Issue #240 Design

## Scope and evidence rule

Extend the existing bounded model probe so one run measures the configured
executor, planner, and classifier roles as a role pair rather than collapsing
their evidence into one aggregate. Keep the change inside `src/model_probe.rs`
and bilingual guide pages. Do not change provider implementations or
`src/config.rs`.

A concrete preset recommendation may be documented only after running the
candidate probe and recording its exact provider/model IDs, local model
digests, candidate build, command, completion bands, and elapsed measurements.
The probe remains a micro-task dialect measurement, not a production-capability
benchmark; the docs must preserve the existing smoke/full-scenario admission
steps and must not infer a quality or speed multiplier from model size.

## Probe contract

- Bump the fixed battery to `model-probe-v3` because its task set changes.
- Keep the existing ten executor tasks and planner JSON-schema task, and add
  one bounded classifier task that selects from a closed candidate list using
  the configured classifier provider/model and its existing bounded call
  override.
- Add the resolved classifier role to the JSON report and card.
- Add per-role measurements derived from task-owned provider-turn evidence:
  task completion counts, a categorical completion band, provider turn count,
  total provider duration, latency statistics, and reported token totals.
  The band describes completion of this fixed probe only; it is not a model
  tier or acceptance verdict.
- Preserve the existing aggregate metrics and task/event shapes. New report
  fields are additive. Include all three model IDs in output basenames so two
  role combinations measured in the same second cannot overwrite each other.

## Documentation and preset

- Update the shared model-probe guide with the v3 battery, exact role-pair
  measurement procedure, interpretation limits, and a bilingual measurement
  record linked from the provider/configuration guides.
- Add matching English and Japanese provider guidance explaining why executor,
  planner, and classifier choices must be measured independently.
- Add matching English and Japanese preset examples for the measured role
  split. Label the exact scope and date, keep fallback/inheritance semantics
  explicit, and point to the committed measurement record.
- Leave the generated config template unchanged: a measured example in the
  guide is not a new built-in default and must never auto-configure users.

## Verification

- Extend focused model-probe tests for the classifier task, role metadata,
  role completion bands/timings, v3 task count, card rendering, and unique
  role-pair filenames.
- Run the focused Rust test, documentation drift/link tests, formatting,
  Clippy, and the full Rust suite because the serialized probe report is a
  shared CLI artifact.
- Build the release binary, run the documented large-only baseline and
  large-executor/small-planner-classifier candidate against the installed
  pinned Ollama models, and record the observed results without rewriting any
  historical evidence.
