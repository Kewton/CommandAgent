# Issue 241 implementation summary

## Implemented behavior

- Planner-step and ultra-planner provider calls now use streaming transport
  whenever the selected client/model supports it, even when visible terminal
  streaming is disabled. Executor and repair calls retain their prior
  `--stream` gating.
- Planner stream chunks remain hidden. Empty transport heartbeats are drained
  without reaching terminal render callbacks.
- Ollama reasoning-only and tool-only stream records now produce empty
  transport heartbeats. This gives the provider worker a cancellation boundary
  before visible answer content without rendering or saving private thinking.
- An interrupt retains the existing honest aborted outcome and event evidence,
  drops the provider worker receiver, and makes the next Ollama stream callback
  unwind the response so the HTTP connection closes.

## Tests

- Added provider-call coverage for hidden planner streaming under
  `--stream off`, unchanged executor batch transport, prompt cancellation, and
  the existing `aborted_by_user` evidence.
- Added Ollama parser coverage for hidden reasoning heartbeats and callback
  cancellation before visible content.
- Added an opt-in PTY regression that confirms a Gate 1 Next.js fix request,
  interrupts the live planner stream, and asserts that Gate 4 appears and the
  fake Ollama socket closes within one second. It also pins hidden planner
  payloads and visible interrupted evidence.

## Compatibility

- No event name, event schema, serialized reply contract, historical evidence,
  or `.anvil/` runtime namespace changed. No corpus fixture update was needed.
