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

## Gemini and OpenAI

Gemini and OpenAI are planned provider adapters. They use XML fallback tool
calls by default unless native tool support is deliberately added later.

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
