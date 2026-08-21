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

## Follow-up propagation: Issue 162 auth retry

### Source and overlap

- Issue #169 follow-up commit `a37495fd` is the verified combined-tree form of
  the Issue #162 auth-retry patch (patch-equivalent to `ea8f8fbd`). This branch
  already contains the earlier #162 commit and #169 run-identity commit
  `0ca9c5cb`, so the follow-up must be applied on top rather than replacing
  either inherited contract.
- The production change defers only automatic session-index revalidation while
  the compose screen has a concrete reconnect target. This prevents a
  background wrong-token 401 from clearing the controlled token before the
  explicit reconnect request can own rejection and retry.
- The source patch also updates the unchanged full-smoke editable-identity
  cardinality from six to seven. Its `smoke.mjs` hunk overlaps Issue #156's
  provider-propagation smoke mode, so resolution must retain both the seven
  control assertion and every OpenAI/Gemini request, identity, and delegated
  CLI argv assertion.

### Propagation plan

1. Commit this design amendment before production integration, then cherry-pick
   `a37495fd` and resolve only demonstrated overlap semantically.
2. Preserve the existing `planner_provider` API field, atomic provider pairing,
   independent executor/planner model inputs, and #169 Gate 2/terminal identity
   projection. Preserve every auth, GET-only reconnect, timing, measured-mean,
   root/proxy, terminal, and editable-control acceptance assertion.
3. Rebuild `target/release/commandagent` and run the focused session-index,
   provider-propagation, and feedback/identity browser smokes. Then run the
   unchanged full root/proxy smoke against that release candidate.
4. Run GUI lint/typecheck plus formatting, default and GUI-feature Clippy, and
   default and GUI-feature Rust tests because the shared Trial UI and browser
   harness overlap.

### Follow-up non-goals

- No forced clicks, longer timeouts, ignored 401 responses, disabled automatic
  refresh outside the explicit reconnect state, skipped base path, relaxed
  cardinality, provider/model inference, or acceptance-gate extension.
