# Issue 237 Implementation Summary

## Outcome

Ollama `/api/chat` requests now always include `options.num_ctx`. The production
provider factory replaces the explicit default with the resolved
`Config.context_budget`, so Ollama loads the model with the same context window
that CommandAgent uses for conversation budgeting.

Doctor now emits a `config.context_budget` check. Its human message shows the
resolved token budget and Ollama `num_ctx` mapping, while JSON details expose the
numeric `value`, `ollama_num_ctx`, affected Ollama roles, and existing source
provenance.

## Changes

- Added the explicit Ollama `num_ctx` request option and a context-budget builder
  without changing the existing `OllamaClient::new` signature.
- Wired `Config.context_budget` into Ollama clients created for executor and
  planner calls.
- Added unit and CLI-doctor assertions for request serialization, default
  behavior, resolved budget display, source metadata, and role mapping.
- Updated the existing `cm4-ollama-think` request-byte corpus fixtures so the
  only request behavior change is the context-window option.
- Documented the `context_budget` to `num_ctx` mapping and `ollama ps CONTEXT`
  observation in the English and Japanese provider guides.

No event schema, conversation trimming rule, historical evidence, or `.anvil/`
runtime namespace changed.
