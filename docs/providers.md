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
