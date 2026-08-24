# Issues #152, #171, and #178 design: residual Trial compose consistency

## Scope and existing behavior

- This combined row owns the residual GUI portion of closed Issue #152 plus
  open Issues #171 and #178. It does not change Issue lifecycle state, terminal
  code, server contracts, event schemas, historical evidence, or `.anvil/`
  runtime state.
- The merged Issue #152/#149 server slice (`643456f4`) already provides the
  read-only `GET /api/provider-models?provider=...` endpoint. Provider pins also
  already stay aligned when the GUI provider changes. This row only consumes
  the discovery endpoint and adds the remaining picker/warning behavior.
- Trial currently filters pack choices to `create`, but its `?pack=` effect
  accepts any selector before that filter. A non-`create` selector can
  therefore remain in `spec.pack` while the select visibly shows no choice.
- The extension catalog and pinned-pack wizard can link every otherwise
  eligible pack into Trial even though Trial only supports `create` packs.
- The sample preset still injects the English goal
  `Create a CLI --pattern filter command`.

## Model candidates and warnings

- When the selected provider is `ollama` or `lm-studio`, request its exact model
  IDs from the existing provider-model endpoint. Abort or ignore obsolete
  requests when the provider changes.
- Attach the returned IDs to both model inputs through native `datalist`
  controls. The inputs remain free-form, so an empty result or failed request
  never blocks Gate 1.
- Only show an unknown-model warning when discovery returned at least one
  candidate and the nonempty exact input is not among them. Keep the existing
  provider-change reminder and provider model hint.
- Cloud providers retain free-form inputs without a discovery request or
  candidate-membership warning.

## Pack handoff consistency

- Resolve `?pack=` only against pack options whose intent is `create`. A valid
  match sets both the pack profile and selector. Any missing or incompatible
  selector explicitly leaves `spec.pack` null and displays a non-blocking
  Japanese explanation beside the pack select.
- Clear the handoff warning when the user changes the profile or pack.
- Show catalog and wizard Trial links only for `create` packs. Other intents
  remain visible and manageable in the extension UI, but cannot create a
  misleading Trial handoff.

## Japanese sample goal

- Use `--pattern で行を絞り込む CLI コマンドを作ってください` for the
  `python-cli` sample and repeat that goal in the getting-started explanation.
- Keep the existing explicit `python-cli` profile and `cli-assist@1.0.0` pack
  preset. Verify that the resulting Gate 1 identity remains `create` ×
  `filter`.

## Verification

- Extend the existing browser smoke assertions for the Japanese sample,
  `create`/`filter` identity, model datalist candidates, unknown-model warning,
  and incompatible pack query normalization.
- Run GUI typecheck, lint, build, the focused two-base-path browser smoke, the
  GUI source guard, formatting, and the full Rust test suite because the smoke
  and repository contract guards are shared CI surfaces.
