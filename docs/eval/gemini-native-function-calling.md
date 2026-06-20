# Gemini Native Function Calling

Date: 2026-06-20

## Scope

This change moves Gemini executor sessions from XML fallback by default to
Gemini native function calling, while keeping XML fallback as a compatibility
and downgrade path.

Responsible layer:

- provider request serialization
- provider response parsing
- provider-independent tool schema
- minimal-loop transcript metadata needed to preserve native tool call/result
  pairs

Out of scope:

- planner/profile/verifier/repair behavior
- OpenAI native tool calls
- hidden retries or provider/model-specific recovery policy
- CI tests that require `GEMINI_API_KEY`

## Baseline

Commit at implementation time: `60e7340`

The worktree was already dirty from ongoing MVP changes, so this report is
implementation evidence rather than clean release evidence.

Targeted baseline before edits:

```text
cargo test providers::      -> pass, 30 tests
cargo test agent::minimal_loop -> pass, 31 tests
```

Relevant baseline shape:

- `Provider::Gemini` defaulted to XML fallback.
- `src/providers/gemini.rs` sent `contents` and `systemInstruction`, but not
  `tools.functionDeclarations` or `functionResponse`.
- `ToolSpec` had no shared JSON argument schema; Ollama built tool parameters
  privately.
- `ChatMessage` and `ToolCall` did not preserve function-call id/name metadata.
- The minimal loop could downgrade native mode to XML fallback only when a
  tool-call parse failure was represented after `ChatResponse`.

## Implementation Summary

Implemented:

- shared tool argument schemas on `ToolSpec`
- Ollama reuse of the shared schema
- optional `ToolCall.id`
- provider-independent `ChatMessage` metadata for assistant tool calls and tool
  results
- Gemini request `tools.functionDeclarations` in native mode
- Gemini response `functionCall` parsing
- Gemini history serialization for assistant `functionCall` and tool
  `functionResponse`
- Gemini transport adapter for REST-specific schema constraints:
  `additionalProperties` is removed from `functionDeclarations.parameters`
  while the internal shared schema remains unchanged
- Gemini 3 `thoughtSignature` preservation on replayed `functionCall` parts
- Gemini native capability default
- XML fallback compatibility for Gemini and OpenAI
- bounded provider parse evidence for malformed native `functionCall` shape,
  routed into the existing parser-feedback/native-to-XML fallback path

Not implemented:

- OpenAI native tool calls
- provider-specific repair behavior
- additional retry loops

## Targeted Checks

After implementation:

```text
cargo test providers::         -> pass, 36 tests
cargo test agent::minimal_loop -> pass, 32 tests
```

New coverage includes:

- Gemini native payload contains `functionDeclarations`.
- Gemini XML fallback payload omits native tools.
- Gemini native `functionCall` response becomes `ToolCall` with preserved id.
- Gemini assistant `functionCall` and tool `functionResponse` history serialize
  without XML text.
- Gemini native history preserves `thoughtSignature` on replayed
  `functionCall` parts.
- malformed native `functionCall` shape becomes bounded parse evidence.
- provider parse evidence downgrades the next minimal-loop request from native
  mode to XML fallback without executing a malformed tool call.

## Interpretation

The targeted failure class was malformed XML fallback tool output from Gemini,
for example missing `Write.path` or missing tool name. Native function calling
reduces reliance on prose/XML/JSON formatting for ordinary Gemini tool calls.
XML fallback remains available if native parsing fails or if compatibility
mode is selected.

This is aligned with the design philosophy because the change is bounded to the
provider/tool-call boundary, uses deterministic schemas, does not add hidden
continuation, and leaves planning, profiles, verification, and repair contracts
unchanged.

## Live Gemini UAT

Manual live Gemini UAT was run against `gemini-3.1-flash-lite` with `.env`
loaded externally:

```text
source .env
target/release/commandagent --yes --context-budget 65536 \
  --model gemini-3.1-flash-lite \
  --provider gemini 'Create hello.txt containing exactly ok.'
```

Observed sequence:

1. First live attempt failed before tool execution because Gemini REST rejected
   `additionalProperties` in `functionDeclarations.parameters`.
2. After adding Gemini schema conversion, the model successfully created
   `hello.txt`, then the next request failed because replayed `functionCall`
   history lacked `thoughtSignature`.
3. After preserving `thoughtSignature`, the focused UAT completed:

```text
OK. I have created `hello.txt` with the content "ok".
HELLO_CONTENT=ok
```

Successful workspace:

```text
/private/tmp/commandagent-gemini-native-a6vYL1
```

This confirms the minimal native Gemini tool-call roundtrip. It does not claim
Next.js app quality or visual/gameplay success; those remain separate eval
contracts.
