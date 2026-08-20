# Issue 156 design: keep the Trial planner provider aligned

## Context

- The Trial form exposes one provider selector for both execution roles, but
  its state update changes only `provider`. The separately serialized
  `planner_provider` therefore remains at its initial `ollama` value when an
  operator selects OpenAI or Gemini.
- Gate 1 already freezes both provider fields in `ExecutionPins`, and GUI
  delegation already passes the frozen planner pin to CLI
  `--planner-provider`. The server contract is correct and does not need a new
  field or endpoint shape.
- Required predecessor #169 contains #162. Their additive session timing and
  identity response fields share the Trial files and must remain intact.
  Predecessor #206 is an independent workspace-confinement change that must
  also be present in the combined branch.

## Smallest coherent change

1. Make the existing `provider` update atomic: update both `provider` and
   `planner_provider` to the selected value while leaving `model` and
   `planner_model` untouched. Continue serializing the existing
   `planner_provider` field in proposal and create requests.
2. Keep the UI's explicit model inputs and current warning that provider
   changes do not rewrite model IDs. No second provider selector or server-side
   inference is introduced.
3. Add a focused browser smoke mode that selects OpenAI and Gemini through the
   real UI, completes Gate 1 against `gui_server`, delegates to a local probe
   binary, and verifies both the create-request `planner_provider` field and
   the resulting `--provider` / `--planner-provider` CLI arguments.
4. Update the Trial guide to state that the single provider selection applies
   to both roles while their model IDs remain independently entered.

## Verification strategy

- Run JavaScript syntax, GUI typecheck, lint, and build checks.
- Run the new provider-propagation browser smoke for both supported base paths.
- Run focused GUI source/server tests, then repository formatting, Clippy, and
  default plus GUI-feature test suites because the final tree includes shared
  predecessor Rust changes and the GUI delegation contract is exercised.

## Non-goals

- No rename or removal of `planner_provider`, API/schema migration, automatic
  model-ID selection, provider call, credential probe, event rewrite, or
  `.anvil/` runtime migration.
