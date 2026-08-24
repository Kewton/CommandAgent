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

## Follow-up propagation from Issue 162

- Recorded the combined-tree propagation design in commit `69fbc386` before
  changing production code.
- Integrated Issue #169 commit `a37495fd` as patch-equivalent commit
  `5d6773f9`. The Trial session index now defers automatic revalidation only
  while the compose screen owns a concrete reconnect target, so an automatic
  wrong-token response cannot consume the explicit reconnect retry flow.
- Kept manual session-index refresh and direct session access active and
  GET-only. A rejected reconnect token is still removed, the retry action is
  enabled, and the successful retry performs the authenticated session and
  artifact reads.
- Preserved Issue #156's atomic executor/planner provider update, independent
  executor/planner model inputs, existing `planner_provider` API shape, and all
  OpenAI/Gemini request plus delegated CLI assertions.
- Preserved Issue #169's frozen Gate 2, reconnected, terminal, closed, and next
  run identity behavior. The unchanged full smoke now checks all seven
  editable identity controls for the next draft run.

No acceptance gate, timeout, base-path case, or assertion was weakened,
skipped, forced, or extended during propagation.

## CI follow-up: GUI session-file error size

- Cherry-picked only Issue #160 code commit `714017ca` as `f5c60bbb`; Issue
  #160 report commit `1f28c021` and all Issue #160 report files remain absent
  from this branch.
- Wrapped the session-file handler's error `Response` in `Box<Response>` and
  returned that same response from `IntoResponse`. The conversion does not
  rebuild the response, so its status, headers, and JSON body bytes remain
  unchanged while satisfying the Rust 1.98 result-size lint without an allow.
- Left authentication, request routing, path confinement, artifact/event read
  limits, and all symlink rejection logic unchanged.
- Re-ran the Issue #156 provider source guard and root/proxy browser smoke to
  confirm the CI-only server change did not alter provider/planner propagation
  or independent executor/planner model IDs.
