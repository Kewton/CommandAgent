# Issue #251 implementation summary

## Outcome

CommandAgent now accepts `--provider openai-compatible --base-url <URL>` for
generic OpenAI-compatible servers such as vLLM and llama.cpp. Executor,
planner, and preset-only classifier roles retain the `openai-compatible`
identity in diagnostics and events while sharing LM Studio's established
OpenAI-compatible Chat Completions/Responses transport.

## Implementation

- Added a leaf public-CLI adapter with `openai-compatible`, `--base-url`, and
  `--api-key-env`, and routed binary parsing, dynamic completion, man-page
  generation, and CLI documentation drift checks through that adapter. This
  preserves the approved prohibition on editing `src/cli.rs`.
- Added role-aware generic-provider resolution for flags and presets, strict
  HTTP(S) base-URL normalization, process-environment-only bearer-token
  loading, fail-closed environment-variable-name validation, remote timeout
  defaults, and existing W3 unknown-key diagnostics for the two new preset
  keys.
- Reused the LM Studio implementation as a shared configurable transport while
  keeping LM Studio's endpoint, token, labels, and diagnostics backward
  compatible. Generic requests retain their own labels, events, error hints,
  authentication, native-tool support, XML fallback, metadata, and Responses
  state.
- Added role-aware `ChatClient` construction and an Ollama-thinking capability
  query. Provider calls now ask the client capability instead of comparing the
  provider enum, and classifier overrides use the classifier role explicitly.
  The existing boxed-clone cancellation and timeout boundary remains intact.
- Added generic startup and doctor model probes plus status, banner, summary,
  REPL pin, and event labeling through `Config::provider_label`.
- Added bilingual CLI/configuration/provider documentation. The generic
  provider section is additive and placed after LM Studio so Issue #240's
  preceding role recommendations can merge without being replaced.

## Tests

- Added parser/help tests for the augmented command and config tests for role
  inheritance, preset parsing, precedence, diagnostics, URL/key validation,
  and remote timeout selection.
- Added shared-transport mock tests for bearer authentication, native tool
  calls, metadata, event identity, and redaction.
- Added an actual delayed-loopback cancellation test through the provider-call
  chokepoint.
- Added a binary integration test that runs `--doctor --json` against a mock
  `/v1/models` endpoint and proves generic diagnostics do not masquerade as LM
  Studio.
- Updated documentation drift coverage so the bilingual public flag inventory
  is derived from the binary's augmented command.

## Predecessor and scope audit

- Inspected Issue #230 commit `a450f53f`, Issue #240 commit `a29e7b3a`, and
  Issue #231/#151 commit `df36fa9e`; none was assumed to be an ancestor of this
  branch.
- Did not edit `src/cli.rs`, GUI files, historical run evidence, migration
  evidence, or the live `.anvil/` namespace.
- Preserved provider event names and schemas; `openai-compatible` is an
  additive provider identity.
