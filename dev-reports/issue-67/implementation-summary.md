# Issue #67 Implementation Summary

## Implemented

- Added the unauthenticated, read-only `GET api/trial-options` route in a new
  GUI-server leaf module. Its profile entries are generated directly from
  `admitted_profiles()` and include display labels and scope descriptions.
- Centralized the GUI server's admitted provider list in that module. The
  options response and `SessionSpec` validation now consume the same four
  provider IDs, and every provider entry carries exact-model guidance.
- Removed the demo Goal and both `qwen3:8b` browser defaults. Goal, executor
  model, and planner model now start empty and are validated locally before a
  proposal request; untouched forms report Goal guidance before token
  guidance.
- Replaced client-side profile/provider option literals with the server
  response. The selected profile description is visible in the form, and a
  provider change displays a warning that the executor model is not rewritten
  plus the selected provider's server-supplied model hint.
- Added stable Trial profile/provider/model test IDs. The two-base-path smoke
  waits for the server-derived `python-cli` option and explicitly fills Goal,
  executor model, and planner model.
- Updated the user guide with empty-field behavior, option discovery,
  authentication boundaries, provider-model guidance, and smoke expectations.

## Tests and compatibility

- Added a GUI server integration test proving that the unauthenticated options
  response's profile IDs exactly equal `admitted_profiles()`, provider IDs
  retain the admitted set, and every option includes non-empty guidance.
- Extended the GUI read-only guard to pin local validation, dynamic options,
  empty defaults, provider warnings, and explicit smoke fills. Its existing
  recursive no-provider-call/no-runner-call protection remains green with the
  new module.
- No provider is contacted by option discovery. Session execution still occurs
  only through the existing confirmed CLI delegate.
- No event name/schema, recovery behavior, corpus fixture, historical
  evidence, or `.anvil/` runtime namespace changed.

## Predecessors

Issues 63 (`4313d7ef`), 64 (`7fcb0dbe`), and 66 (`d6f0dec5`) were inspected as
parallel non-ancestor predecessor commits. Their polling, workspace-lease, and
post-run lifecycle changes were not duplicated or merged into this independent
Issue patch; the overlapping edits remain narrowly scoped for later normal
integration.
