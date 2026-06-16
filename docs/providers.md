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

## Gemini

Gemini uses the Generative Language API `models/{model}:generateContent`
endpoint. System messages are sent as `systemInstruction`; user and tool
messages use the `user` role; assistant messages use Gemini's `model` role.

Gemini provider smoke is opt-in because it requires network access and
`GEMINI_API_KEY`:

```bash
GEMINI_API_KEY=... GEMINI_MODEL=gemini-3.5-flash scripts/provider_smoke_gemini.sh
```

Gemini uses XML fallback tool calls by default.

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

OpenAI uses XML fallback tool calls by default.

The canonical fallback format is:

```xml
<commandagent_tool_call>{"name":"Read","args":{"path":"Cargo.toml"}}</commandagent_tool_call>
```

The payload must be a JSON object. Supported name keys are `name`, `tool`, and
`tool_name`; supported argument keys are `args` and `arguments`.

If native tool parsing fails during a session, the active tool mode is
downgraded to XML fallback for the rest of that session. The loop implementation
will own the session state transition; the parser exposes the deterministic
mode transition helper.
