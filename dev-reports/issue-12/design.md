# Issue #12 Design

## Goal

Stream provider text into the interactive REPL without changing the completed
`AssistantReply` consumed by tool parsing, session persistence, event output,
or evaluation. Preserve the existing blocking and fake-client paths.

## Predecessor integration

- Integrate Issue #11 so the stateful Markdown renderer used for streaming is
  the table/list/highlighting-capable renderer that the scheduler approved.
- Integrate Issue #14, which already contains Issues #13 and #15, so streaming
  writes use the resize-safe fixed-footer scroll region and coexist with the
  in-flight input queue and current CommandAgent prompt.

## Provider API and protocols

- Keep `ChatClient::chat` unchanged and add an opt-in `supports_streaming` plus
  `chat_stream` callback API. Default trait behavior remains non-streaming, so
  fake clients and every existing caller retain their current contract.
- Add `src/providers/streaming.rs` for shared blocking `Read` support. It will
  frame NDJSON and SSE without converting incomplete byte buffers to strings,
  so a UTF-8 code point split across reads is decoded only after a complete
  line/event is available.
- Ollama sends `stream: true`, combines NDJSON message deltas and terminal
  timing/token fields, then applies XML fallback to the completed text.
- OpenAI sends a streaming Responses request, emits
  `response.output_text.delta` text, and uses the completed response event (or
  accumulated output-item state) to construct the same reply shape.
- Gemini uses `streamGenerateContent?alt=sse`, maps the existing conversation
  and tool schema to GenerateContent request fields, and combines candidate
  text, function calls, and final usage metadata.
- HTTP/status/parse failures may retry only while no text delta has been
  delivered. Once output is visible, disconnects or malformed framing return
  an error with the partial output left intact.

## Call and timeout behavior

- Extend the existing synchronous provider worker channel with chunk messages.
  The main thread continues checking Esc/Ctrl+C and the configured deadline at
  chunk/250 ms boundaries; no async runtime is introduced.
- `chat_timeout_secs` remains the wall-clock cap for the whole provider turn,
  including all pre-first-token retries. A dropped receiver stops a streaming
  provider at its next callback boundary.
- Streaming is effective only for an interactive TTY REPL, when enabled by the
  resolved setting, and when the concrete client opts in. `--prompt`, other
  one-shot actions, non-TTY execution, and fake clients therefore continue to
  call `chat`.

## Configuration

- Add `--stream on|off` and `stream = "on"|"off"` at top-level or in a
  preset, using the existing flag > preset > config file > default precedence.
- Default the preference on for `Action::Repl` and off for one-shot actions;
  the runtime TTY/client gates are stricter than that preference.
- Report the source beside other resolved fields and document that the timeout
  covers the full stream.

## TUI rendering

- Give a model-call `UiGuard` a lazily active incremental
  `TerminalMarkdownRenderer` session. On its first non-empty raw chunk it drops
  and joins the spinner before writing rendered bytes. Finish/error/interrupt
  flushes buffered Markdown and terminates the partial line cleanly.
- Keep `<think>` state in the existing Markdown renderer across chunks. The
  complete raw reply remains separate from display state, preserving XML
  fallback and all persistence/event payloads.
- Continue writing body output to stdout while the footer owns its reserved
  rows through DECSTBM/save-restore. Do not freeze the footer during generation,
  because Issue #14 must continue echoing queued input.

## Verification

- Unit-test byte-split UTF-8, multiline SSE, malformed frames, and NDJSON.
- Add protocol fixtures for Ollama, OpenAI, and Gemini accumulation, including
  tool calls, usage/timing, and retry-after/before-first-token behavior.
- Add incremental Markdown equivalence and cross-chunk `<think>` coverage, plus
  spinner-before-body ordering and partial flush/error/cancel coverage at the
  call/TUI boundary where practical.
- Run focused provider/config/TUI tests, then formatting, all-target clippy,
  and the complete Rust suite because shared provider and TUI contracts change.
