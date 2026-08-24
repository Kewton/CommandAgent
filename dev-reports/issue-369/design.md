# Issue #369 design: group Trial provider/model controls by role

## Current behavior

The Trial compose form renders execution and planning provider/model controls
as four independent children of a shared two-column grid. Although their DOM
order is provider then model for each role, desktop auto-placement puts each
pair on different visual rows alongside unrelated controls. The mobile grid
collapses to one column, but there is no semantic role container or accessible
group name around either pair.

The existing state and API behavior is already correct: provider updates do not
rewrite model IDs, local model discovery and datalists are scoped by role,
unknown-model warnings are role-specific, and Ollama thinking follows either
role's provider selection.

## Design

- Wrap the execution and planning pairs in native `fieldset` elements with
  visible `legend` group names. Keep the existing visible field labels and test
  IDs so each control retains its accessible name and automation contract.
- Give each role group a two-column inner grid on desktop, making Provider and
  Model one logical and visual row. At the existing mobile breakpoint, collapse
  only the inner grid so each provider is immediately followed by its model;
  keep Executor before Planner in both DOM and visual order.
- Preserve the current controls, handlers, datalist IDs, warnings, discovery
  hooks, and Ollama thinking selector without state or API changes.
- Update the Trial guide to state the grouped desktop/mobile presentation.

## Focused verification

- Extend the synthetic compose browser probe, which already runs for `/` and
  `/proxy/commandagent/`, to assert fieldset/legend semantics, accessible labels,
  desktop row alignment, mobile provider-before-model ordering, role boundaries,
  and real Tab order.
- Extend the Rust GUI read-only/source guard to pin the semantic markup,
  responsive grid contract, and browser assertions.
- Run GUI syntax, lint, typecheck, build, and the focused two-base-path smoke;
  then run formatting, Clippy, and the Rust test suite because the shared GUI
  smoke and repository guard are changed.

## Compatibility

No request, response, event, Gate 1 identity, CLI argument, or `.anvil/`
runtime schema changes are needed. The change is presentation and semantic
grouping only.

## CI follow-up: delegated argument synchronization

The Linux GUI Dashboard exposed a deterministic race in the intent integration
test: shell redirection creates `delegated-args.txt` before `printf` writes its
contents, while the test treated file existence as proof that delegation had
finished. The production delegate already waits for the child process and only
then releases the Trial workspace lease.

- Keep production provider/model and intent delegation unchanged.
- Make the fixture expose an empty argument file briefly before writing, so the
  former synchronization bug is covered deterministically.
- Read delegated arguments only after `/api/trial-workspace` reports `idle`,
  the public completion boundary backed by child-process exit.
- Retain exact assertions for the intent flag and add the existing executor and
  planner provider/model flag pairs to the same focused regression.
