# Issue #73 Implementation Summary

## Implemented

- Reworked the existing Gate 1 `card_markdown` presentation into Japanese
  reader-facing sections for the request, work type, required checks,
  comparable-run results, file boundary, models, optional pack, and exact
  confirmation ID. Python CLI C1-C4 each have contract-aligned explanations;
  the other admitted create-profile check sets are described as well.
- Added a small escaped React renderer for the server-provided markdown. The
  Trial page now renders it under `data-testid="gate-one-card-markdown"`
  instead of reconstructing a second route/check card from raw identity fields.
- Replaced the raw measured-price/window presentation with Japanese time/cost,
  sample-count, comparable-pass, and filesystem-boundary explanations. The
  card hash remains visible as a confirmation ID with change-detection guidance.
- Replaced the Terminal assurance-as-heading fallback with a Gate 3/Gate 4
  result heading. Assurance values are mapped to a separate Japanese evidence
  explanation, so `static` cannot appear as the verdict heading.
- Reworded the D-3d follow-up controls in Japanese without changing directive
  proposal hashing, confirmation, or continuation behavior.
- Updated the CLI contract and GUI/first-loop documentation to record the
  reader-facing aliases without changing any contract ID or meaning.

## Tests and browser evidence

- Extended presentation tests to pin the Japanese C1-C4 descriptions,
  comparable-run result, pack state, confirmation ID, and absence of the old
  `Card hash`/`Route`/`Checks`/`Value tag` labels.
- Extended GUI server integration coverage to prove the proposal still returns
  `confirmation_required`, the unchanged C1-C4 identity IDs, a card containing
  the same hash, and the new reader-facing markdown.
- Extended the GUI source guard for the markdown test ID, escaped renderer,
  unchanged confirmation boundary, new Terminal projection, and removal of the
  targeted internal copy.
- Extended `smoke.mjs` to read the markdown through its test ID, verify the
  Japanese C1-C4 and comparable-run wording, retain the updated
  `gate_1.visible_text`, and prove the Terminal heading differs from assurance.
  The real Ollama/Playwright lap passed for both `/` and
  `/proxy/commandagent/`.

## Compatibility

`ConfirmationIdentity`, its serialized card-hash input, Trial route names and
JSON field set, exact-hash dispatch enforcement, explicit checkbox, CLI-only
delegation, directive APIs, event names/schemas, corpus contracts, historical
evidence, and `.anvil/` state are unchanged. `card_markdown` is presentation
content and now carries the approved Japanese wording; opaque IDs and paths are
preserved. No corpus fixture or state migration was required.

The required sibling predecessor commits #64, #67, #68, #69, #70, #71, #76,
and #80 were inspected before editing. Their independent lease, options,
phase, feedback, artifact, session-index, localization/runtime-status, and
polling changes were not merged or duplicated. The Issue #76 Japanese/no-i18n
decision was applied directly to every reader-facing string added or replaced
here.
