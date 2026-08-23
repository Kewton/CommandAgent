# Issue #353 design: separate GUI Trial providers by role

## Current behavior

The Trial request already carries `provider` and `planner_provider` separately,
and the server freezes them into the Gate 1 identity before delegating them as
`--provider` and `--planner-provider`. The compose hook currently defeats that
contract by rewriting `planner_provider` whenever the single provider selector
changes. Both model inputs also share the executor provider's discovered-model
list.

The required Issue #352 predecessor was inspected at commit `8e4efda2`. Its
overlapping GUI smoke change follows the new per-session execution workspace;
it does not change the provider request, Gate 1 identity, or CLI flag contract.

## Design

- Keep the existing `trial-provider` control as the execution-provider selector
  and add a distinct `trial-planner-provider` selector backed by the same
  admitted provider options.
- Make ordinary `SessionSpec` field updates independent, so selecting either
  provider changes only its own field and continues to invalidate any existing
  Gate 1 proposal.
- Fetch executor and planner model candidates from their respective provider
  selections. Give the inputs separate datalists and evaluate unknown-model
  warnings against the corresponding candidate set.
- Leave the server and schemas unchanged: the existing Gate 1 and delegate code
  already map the two request fields to the correct frozen identity and CLI
  flags.
- Update the Trial guide with the role-to-flag mapping and provider-scoped model
  discovery behavior.

## Verification

- Update the source guard to require two provider controls, independent updates,
  and role-specific datalists.
- Extend the synthetic compose regression to select different local providers
  and prove each model input receives only that provider's candidates.
- Extend the provider-only browser smoke to select different execution/planning
  providers and assert the request body, Gate 1 card, frozen run identity, and
  delegated CLI flags all retain the distinction.
- Run focused GUI/static checks first, followed by GUI lint, typecheck, the
  provider-only smoke, and broader repository checks required by the changed
  Rust test guard.
