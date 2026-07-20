# Issue 48 design

## Problem

With terminal streaming enabled, planner provider chunks are sent through the
same markdown stream used for executor replies. Step-plan JSON and UltraPlan
machine output therefore enter REPL scrollback before validation and push the
accepted command, plan card, and activity breadcrumbs off screen.

## Constraints

- Suppress terminal rendering for both `PlannerStep` and `PlannerUltra` scopes.
- Keep provider transport streaming, final reply assembly, cancellation,
  telemetry, event schemas, spinner lifetime, breadcrumbs, and footer cleanup
  unchanged.
- Keep `Executor` and `Repair` streaming behavior unchanged.
- Avoid growing the planner runner chokepoint for a provider-call policy.

## Design

Add a scope predicate in `src/provider_call.rs` that decides whether received
chunks may be forwarded to the caller's rendering callback. The worker still
uses `chat_stream` whenever streaming is configured and supported, so provider
request/response semantics remain stable. For planner scopes the receive loop
drains chunks without invoking the callback; the completed `AssistantReply` is
processed normally. For executor and repair scopes the callback path is
unchanged.

Because planner callbacks are not invoked, `UiGuard` keeps the spinner active
until the provider turn completes or is interrupted. Finishing an empty
terminal markdown stream is a no-op, and dropping the guard performs the same
spinner/footer restoration as before.

## Tests

- Add provider-call unit coverage for both planner scopes to prove transport
  streaming remains enabled while rendering callbacks receive no chunks.
- Retain the existing executor streaming unit test as the unchanged-behavior
  assertion.
- Update the PTY streaming regression to assert that planner JSON is absent
  while the planner breadcrumb, spinner cleanup, footer restoration, and prompt
  recovery remain visible.
- Exercise both `/plan-steps <goal>` and `/ultra-plan-run <goal>` planner paths
  in PTY coverage, including Esc interruption cleanup for an in-flight planner
  turn.

## Event compatibility

No event emission, event name, key, or value changes. The change affects only
whether planner chunks are forwarded to terminal rendering.
