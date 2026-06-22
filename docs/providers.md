# Providers

CommandAgent uses a thin provider contract. The provider layer owns transport
and response parsing. It does not own planning, repair, profiles, or tool
execution.

## Ollama

Ollama supports:

- `/api/tags` model listing
- `/api/chat`
- native tool calls when the request mode is `Native`
- retry and timeout at the transport boundary

Optional local smoke:

```bash
OLLAMA_HOST=http://127.0.0.1:11434 scripts/provider_smoke_ollama.sh
```

This smoke is intentionally not part of `scripts/eval_smoke.sh` because it
requires a running local service.

Default local endpoint:

```bash
OLLAMA_HOST=http://127.0.0.1:11434
```

For client use, bare host values are normalized. `OLLAMA_HOST=0.0.0.0` is
treated as `http://127.0.0.1:11434` because `0.0.0.0` is a server bind address,
not a useful client target.

## Gemini

Gemini uses the Generative Language API `models/{model}:generateContent`
endpoint. System messages are sent as `systemInstruction`; user and tool
messages use the `user` role; assistant messages use Gemini's `model` role.

Gemini provider smoke is opt-in because it requires network access and
`GEMINI_API_KEY`:

```bash
GEMINI_API_KEY=... GEMINI_MODEL=gemini-3.5-flash scripts/provider_smoke_gemini.sh
```

Gemini uses native function calling by default. In native mode, CommandAgent
sends built-in tools as `tools.functionDeclarations`, parses Gemini
`functionCall` parts into the shared `ToolCall` contract, preserves the
function call id/name in transcript metadata, and sends tool results back as
Gemini `functionResponse` parts on the next request. The provider owns only
this request/response serialization; it does not choose repair strategy,
increase retries, or execute tools.

Gemini's REST schema is not identical to the shared internal tool schema. The
Gemini provider removes unsupported JSON-schema fields such as
`additionalProperties` when serializing `functionDeclarations`; the internal
schema remains unchanged for other providers. Gemini 3 function-calling history
also requires preserving `thoughtSignature` on model `functionCall` parts and
returning it unchanged in the next request history.

XML fallback remains a compatibility path. If Gemini response text contains
`<commandagent_tool_call>...</commandagent_tool_call>` blocks, they are still
parsed and returned as `ChatResponse.tool_calls` with the XML removed from
assistant content. Malformed native `functionCall` shape is surfaced as bounded
tool-call parse evidence so the minimal loop can use its existing parser
feedback and native-to-XML fallback downgrade. True transport, HTTP, and
unparseable response-body failures remain provider/model errors.

Example mixed planner/executor usage:

```bash
commandagent \
  --provider ollama \
  --model qwen3.6:35b-a3b-coding-nvfp4 \
  --planner-provider gemini \
  --planner-model gemini-3.5-flash
```

## OpenAI

OpenAI uses the Responses API `/responses` endpoint. User/system/tool messages
are encoded with `input_text`; previous assistant messages are encoded with
`output_text`. This distinction is part of the provider contract because sending
assistant history as `input_text` causes API request failures.

OpenAI provider smoke is opt-in because it requires network access and
`OPENAI_API_KEY`:

```bash
OPENAI_API_KEY=... OPENAI_MODEL=gpt-5.4-mini scripts/provider_smoke_openai.sh
```

OpenAI uses XML fallback tool calls by default. Response text from
`output_text` and `output[].content[].text` is parsed for
`<commandagent_tool_call>...</commandagent_tool_call>` blocks, and parsed blocks
are returned as `ChatResponse.tool_calls` with the XML removed from assistant
content. Malformed XML-like tool-call blocks are reported as provider parse
errors. The minimal loop renders parsed XML fallback calls back into assistant
history on the next request so the provider can see the tool call that produced
the following tool result.

OpenAI can also be used as only the planner:

```bash
commandagent \
  --provider gemini \
  --model gemini-3.1-flash-lite \
  --planner-provider openai \
  --planner-model gpt-5.4-mini
```

## XML Fallback Tool Calls

The canonical fallback format uses the CommandAgent tag and an `args` JSON
object:

```xml
<commandagent_tool_call>{"name":"Read","args":{"path":"Cargo.toml"}}</commandagent_tool_call>
```

The payload must be a JSON object. Supported name keys are `name`, `tool`, and
`tool_name`; supported argument keys are `args` and `arguments`. New prompts,
docs, and tests should use `args`. `arguments` remains accepted only for
migration tolerance.

After a provider or XML fallback parser returns a tool call, all providers share
the same minimal-loop tool schema contract. Missing required arguments such as
`Write.path`, `Write.content`, or `Bash.command` are rejected as structured
tool-argument failures before any tool mutation. The step runner may expose
that evidence through one bounded tool protocol correction, but providers do
not own that repair policy.

Tool protocol failures now align with the common contract-evidence shape in
the step-runner adapter, but providers still remain transport-only. Provider
modules must not choose repair strategy, alter retry budgets, or add
model-specific behavioral policy. If a provider or XML fallback parse failure
cannot be represented as a safe shared correction action, the step runner emits
provider-transport evidence and stops boundedly instead of putting recovery
policy into the provider.

Native providers consume the same built-in tool argument schemas. `Read`,
`Write`, `Edit`, `Glob`, `Grep`, and `Bash` expose one provider-independent
JSON schema boundary, and providers serialize that schema into their own native
tool declaration format. XML fallback keeps the human-readable examples below
for prompt compatibility.

Built-in argument shapes:

| Tool | Args |
| --- | --- |
| `Read` | `{"path":"README.md"}` |
| `Write` | `{"path":"README.md","content":"text"}` |
| `Edit` | `{"path":"README.md","old":"before","new":"after"}` |
| `Glob` | `{"pattern":"src/*.rs"}` |
| `Grep` | `{"pattern":"TODO"}` |
| `Bash` | `{"command":"cargo test"}` |

If native tool parsing fails during a session, the active tool mode is
downgraded to XML fallback for the rest of that session. The loop implementation
will own the session state transition; the parser exposes the deterministic
mode transition helper.

Live API smoke is manual and opt-in. Do not add provider API keys to automated
tests, default CI, or default eval scripts.
