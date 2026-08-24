# Issue 241 design

## Problem

Planner calls use `chat_with_cancel_and_stream`, but provider transport currently
streams only when visible REPL streaming is enabled. With `--stream off`, an
Ollama planner call uses the non-streaming `/api/chat` response and the detached
worker retains the HTTP request after the caller records an interrupt. This can
delay the confirmed-run failure path and Gate 4 until generation finishes.

## Constraints

- Limit production behavior to the provider-call boundary; do not grow planner
  runner or minimal-loop chokepoints.
- Preserve hidden planner machine output, completed reply assembly, retry rules,
  timeout behavior, event names and schemas, and the existing
  `aborted_by_user` evidence.
- Do not make executor or repair transport stream when terminal streaming is
  disabled.
- Keep unsupported provider/model combinations on their existing batch path.

## Design

Treat planner transport streaming as a cancellation mechanism independent of
terminal rendering. In `src/provider_call.rs`, select `chat_stream` for
`PlannerStep` and `PlannerUltra` whenever the cloned client supports streaming
and the call supplied the stream callback. Executor and repair scopes retain
the existing TTY/configuration gate.

Planner chunks remain suppressed by the existing scope rendering predicate.
The provider worker still sends each chunk through its bounded channel so the
caller drains the response while active. On interruption, the provider-call
loop records the existing honest aborted outcome and drops its receiver. The
next Ollama stream callback then fails, unwinding `parse_chat_stream` and
dropping the blocking response, which closes the HTTP request and stops Ollama
generation. `src/providers/ollama.rs` also maps reasoning-only and tool-only
stream records to empty transport heartbeats. The provider-call loop drains
those heartbeats without rendering them, so explicit thinking remains private
while cancellation does not wait for visible content.

## Tests

- Extend provider-call unit coverage to prove planner scopes use streaming
  transport even when `config.stream` is false, while executor scope remains
  batch in the same configuration.
- Add a focused cancellation test whose streaming worker observes the callback
  channel close after interruption.
- Add a confirmed Gate 1 Next.js fix flow running under `--stream off`. The
  first phase requires a genuine planner-generated StepPlan, and a streaming
  fake Ollama exposes that request's shape and connection lifetime. The test
  will assert Gate 4 appears less than one second after Esc, the HTTP connection
  closes, planner payloads stay hidden, and interruption evidence remains
  present.

## Compatibility

No event emission site, event key, serialized schema, `.anvil/` namespace, or
historical evidence is changed. The corpus contract is unchanged because the
observable event contract remains byte-compatible.
