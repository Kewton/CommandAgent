# Issue 156 implementation summary

## Result

The GUI Trial provider selector now updates the executor and planner provider
pins together. Selecting OpenAI or Gemini therefore keeps the existing
`planner_provider` request field aligned with `provider`, and the confirmed
identity reaches CLI `--provider` and `--planner-provider` with the same value.

Executor and planner model IDs remain separate explicit inputs and are not
rewritten when the provider changes.

## Changes

- Updated the Trial `SessionSpec` state transition so a `provider` change
  atomically updates `provider` and `planner_provider`.
- Kept the existing `planner_provider` field name, serialized request shape,
  Gate 1 pins, and Rust delegation implementation unchanged.
- Added `npm run smoke:provider`, a focused two-base-path browser smoke that:
  - selects OpenAI and Gemini in the real Trial UI;
  - records each create request's existing provider fields;
  - completes Gate 1 through the real `gui_server`;
  - delegates to a local, non-provider-calling probe binary; and
  - asserts the resulting executor/planner provider and model CLI arguments.
- Extended the GUI source guard to pin the planner-provider synchronization.
- Updated the Trial guide to explain the shared provider selection and
  independent model inputs.
- Integrated required predecessors #162, #169, and #206 before implementation;
  their session timing/identity and workspace-confinement changes remain in
  the verified combined tree.

## Compatibility

No API field, event name, schema, confirmation record, or `.anvil/` namespace
changed. In particular, `planner_provider` remains present and is still the
server's source for the confirmed planner pin.
