# Issue #369 implementation summary

## Outcome

The Trial compose form now presents Provider and Model as one role-specific
group for both **Executor / 実行** and **Planner / 計画**. Native fieldsets and
legends expose each group to assistive technology, while the existing labels
continue to name each control.

On desktop, each role's Provider and Model occupy the same row. At the existing
mobile breakpoint, each group becomes one column so the role order is execution
Provider, execution Model, planning Provider, planning Model. The DOM and real
Tab order match that visual order.

## Preserved behavior

- Provider changes still update only their own provider field and never rewrite
  either model ID.
- Executor and planner local-model discovery remain scoped to their respective
  providers and retain the existing datalists and unknown-model warnings.
- Manual cloud-provider model entry and Ollama thinking enable/clear behavior
  are unchanged.
- Existing control test IDs, request fields, Gate 1 identity, and delegated CLI
  flags are unchanged.

## Coverage and documentation

- The synthetic compose browser probe now checks native group/label semantics,
  desktop and mobile geometry, role boundaries, DOM/Tab order, and model
  preservation after both provider changes. The probe runs for `/` and
  `/proxy/commandagent/` and retains its existing discovery, warning, datalist,
  manual-entry, intent, pack, and thinking checks.
- The Rust GUI read-only guard pins the new fieldsets, legends, desktop grid,
  mobile collapse, and browser assertions.
- The GUI getting-started and Trial guides describe the role groups and their
  responsive order.

## Scope

No Rust production code, API/event schema, recovery contract, corpus fixture,
historical evidence, or `.anvil/` runtime state changed. A corpus update was not
needed because this is a semantic/responsive presentation change only.
