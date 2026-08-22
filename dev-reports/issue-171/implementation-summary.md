# Issues #152, #171, and #178 implementation summary

## Outcome

Implemented the approved combined residual GUI scope without changing terminal
code, server schemas, Issue lifecycle state, historical evidence, or `.anvil/`
runtime state.

## Changes

- Added local-provider model discovery to Trial compose using the existing
  `/api/provider-models` endpoint.
  - Ollama and LM Studio candidates populate one native `datalist` shared by
    the executor and planner inputs.
  - Both fields remain free-form.
  - A nonempty ID outside a nonempty discovered catalog produces a visible
    executor/planner warning before Gate 1.
  - Empty or failed discovery leaves manual entry available without a false
    membership warning.
  - OpenAI and Gemini do not issue local discovery requests.
- Made pack query preselection consistent with Trial's `create`-only choices.
  - `?pack=` resolves only to an exact `create` pack.
  - A missing or non-create selector explicitly leaves `spec.pack` null and
    shows `この pack は現在の profile / intent では選べません。`.
  - Changing the profile or pack clears the handoff warning.
- Hid `Trial で使う` from otherwise eligible non-create rows in the extension
  catalog and from pinned non-create packs in the creation wizard.
- Replaced the GUI sample goal with
  `--pattern で行を絞り込む CLI コマンドを作ってください` and showed the
  exact goal on the getting-started card. The CLI documentation retains its
  separate English sample contract.
- Narrowed the documentation drift guard so it pins the existing English CLI
  goal and the new Japanese GUI goal independently.

## Tests

- Extended the two-base-path browser smoke to verify:
  - the Japanese sample reaches a real Gate 1 proposal as
    `python-cli × create × filter`;
  - a synthetic non-create pack has no catalog Trial link, displays no selected
    value, submits `pack: null`, and receives a successful proposal response;
  - discovered candidates attach to both inputs, unknown exact IDs warn, exact
    candidates clear the warning, discovery failure permits manual input, and
    cloud providers skip discovery.
- Extended the GUI source guard for create-only catalog/wizard handoffs, pack
  normalization, datalist ownership, and model warnings.

No corpus fixture changed because no event, recovery, or corpus contract was
modified.
