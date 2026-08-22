# Issue #251 design: generic OpenAI-compatible provider

## Scope and predecessor constraints

Add a generic OpenAI-compatible provider for executor, planner, and classifier
roles, backed by an explicit base URL and an optional process-environment key
name. Keep LM Studio on the same HTTP implementation while preserving its
existing CLI/config behavior. The approved row decision excludes `src/cli.rs`
and GUI files, so a small provider-specific Clap adapter will extend the binary
command with `openai-compatible`, `--base-url`, and `--api-key-env` and then
pass typed provider options into config resolution.

The predecessor branches are complete but are not ancestors of this worktree.
Issue #230 changes CLI safety/tool policy and must remain untouched. Issue #231
changes REPL provider replacement and preserves the existing provider-call
cancellation boundary; the new client must continue to use `boxed_clone` so
that boundary remains effective. Issue #240 adds measured role-selection
recommendations at the start of both provider guides; this change will add its
generic-provider material in the provider matrix and a later standalone
section so those recommendations remain unchanged when the branches combine.

## Configuration and CLI contract

- Accept `openai-compatible` for executor and planner provider flags through a
  leaf CLI adapter, while keeping the existing `Cli` type and its source file
  unchanged. The adapter also exposes the new arguments in binary help and
  completion metadata.
- Accept `provider`, `planner_provider`, and `classifier_provider` values of
  `openai-compatible` in presets, plus the preset keys `base_url` and
  `api_key_env`. Existing unknown-key diagnostics stay fail-closed.
- Resolve CLI values ahead of preset values. Require `base_url` when any role
  selects the generic provider. Normalize an optional trailing `/v1`, require
  HTTP(S), and reject credentials, queries, and fragments. Validate an optional
  environment-variable name and read only that process variable; never accept
  or record a key value in arguments or config files.
- Represent generic role selection separately from the existing closed
  `Provider` enum. Internally it reuses the LM Studio enum slot only as the
  backward-compatible transport identity, while role-aware config label and
  client-selection queries preserve `openai-compatible` in events and output.
  This avoids changing excluded exhaustive GUI matches.

## Provider and capability design

- Extract the LM Studio OpenAI-compatible transport into a shared client whose
  label, endpoint, optional bearer token, and diagnostics are configured at
  construction. LM Studio becomes one configured form; the generic provider is
  the other.
- Preserve Chat Completions and Responses request shapes, native tools, XML
  fallback, response metadata, retries, and non-streaming behavior.
- Add a `ChatClient` capability for Ollama thinking and use it in the provider
  call path instead of comparing `Provider::Ollama`/labels there. Use client
  labels for provider identity and override selection. Provider cancellation
  remains owned by the existing cloned-worker boundary.
- Keep doctor/config provider-specific enum checks where pre-client discovery
  is required, but exclude generic roles from LM Studio probing and add a
  generic `/v1/models` probe plus optional process-key diagnostics.

## Tests and verification

Add focused tests for CLI adaptation/help, preset parsing and validation,
role-aware labels, optional bearer authentication, mock Chat Completions and
Responses integration, native-tool capability, metadata/event identity, and a
cancelled delayed mock request. Run focused provider/config/CLI/doctor tests,
then formatting, Clippy, and the full Rust suite because configuration,
provider identity, and shared call behavior are affected.
