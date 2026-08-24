# Issue #12 Implementation Summary

## Outcome

Interactive TTY REPL turns now stream assistant text incrementally for Ollama,
OpenAI, and Gemini. The completed provider reply remains the single source for
tool parsing, session storage, events, and evaluation, so streaming changes
presentation without changing persisted data contracts.

## Implementation

- Added a provider-independent `ChatClient::chat_stream` callback contract,
  with opt-in support on the three real providers and non-streaming defaults for
  existing and fake clients.
- Added shared blocking NDJSON/SSE framing over `Read`. Framing retains raw
  bytes until a complete record is available, including when reads split a
  multibyte UTF-8 character.
- Implemented Ollama NDJSON, OpenAI Responses SSE, and Gemini
  `streamGenerateContent` SSE accumulation, including tool calls and usage or
  timing metadata. Ollama XML fallback still parses the completed text.
- Applied `chat_timeout_secs` to the entire synchronous provider turn and
  limited retries to failures before the first visible chunk. Errors after
  partial delivery retain the rendered prefix and report a clear stream error.
- Added `--stream on|off` plus top-level and preset config support using the
  existing flag, preset, config, and default precedence. Streaming defaults on
  only for interactive REPL use and is disabled for non-TTY, one-shot, and
  unsupported/fake-client paths.
- Added an incremental terminal Markdown session that preserves renderer state
  across chunks. It removes the spinner before the first non-empty chunk,
  cooperates with the fixed footer, preserves partial output on interruption,
  hides cross-chunk `<think>` blocks, and produces the same final bytes as batch
  rendering.
- Kept changes to the planner and minimal-loop chokepoints to provider-call/UI
  wiring. Existing retry layers now also refuse to retry after a provider has
  emitted partial stream output.
- Documented configuration, provider behavior, timeout scope, and retry
  semantics in the README.

## Tests

- Added byte-level framing tests for split UTF-8, SSE record construction,
  malformed input, and retry eligibility.
- Added provider fixtures for streamed text, tool calls, usage/timing,
  truncation, and request shape.
- Added provider-call tests for chunk delivery, completed-reply equivalence,
  cancellation after a prefix, and preservation of the legacy fake-client
  route.
- Added batch-versus-stream Markdown acceptance coverage with arbitrary UTF-8,
  table, and `<think>` boundaries.
- Added an opt-in PTY acceptance test using a delayed local Ollama stream to
  verify spinner cleanup, incremental body output, footer integrity, prompt
  recovery, and terminal restoration.

No corpus fixture or event-schema update was needed because the completed
assistant reply and all persisted event content remain unchanged.
