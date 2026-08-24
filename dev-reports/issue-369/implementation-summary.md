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

## CI follow-up

GitHub GUI Dashboard job `97545206706` failed at the delegated intent assertion
with an empty argument list. The delegate was already correct: it passes intent,
executor provider/model, and planner provider/model to the CLI, waits for child
exit, and releases the Trial workspace lease afterward. The failure came from
the test observing `delegated-args.txt` as soon as shell redirection created it,
before `printf` had written any bytes.

The focused integration fixture now creates the file, leaves it empty for 100
ms, and then writes the delegated arguments. The test waits for the existing
`/api/trial-workspace` idle completion boundary before reading the file and
asserts the exact intent, executor provider/model, and planner provider/model
flag pairs. This turns the formerly timing-dependent race into deterministic
regression coverage without changing production behavior or weakening a gate.
