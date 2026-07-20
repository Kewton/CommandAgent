# Issue 46 design

## Goal

Make the first interactive REPL session self-diagnosing without turning provider
availability into a startup gate. A stopped or incompletely configured provider
must produce a short, actionable warning while leaving the prompt usable.

## Design

- Add a provider startup-diagnostics leaf module. It runs only for an interactive
  `Action::Repl` when `--offline` is not set. When either configured role uses
  Ollama, it performs exactly one `GET /api/tags` with a two-second connect and
  request timeout, then reports either host reachability or missing configured
  models. Repeated executor/planner model names are de-duplicated.
- Keep startup diagnostics warning-only. The TUI prints warning lines before
  constructing the normal provider clients and always continues to the editor;
  no new event is emitted, so existing event schemas remain unchanged.
- Add one provider-error formatting helper used by Ollama, OpenAI, and Gemini
  blocking and streaming requests. Connection failures, HTTP 404 responses, and
  HTTP 401/403 responses receive fixed `Hint:` lines tailored to reachability,
  model setup, or API-key setup. Original errors remain present for honest
  failure reporting.
- Make missing OpenAI/Gemini key construction errors name the environment or
  workspace `.env` setup paths and `commandagent --doctor`.
- Add a single banner guidance line for `/help` and `/doctor`. Because
  `--ux-demo` renders the shared banner, update its scripted-output assertion and
  committed SVG excerpt too.

## Compatibility and scope

- Do not change configuration, event names, event fields, or the live `.anvil/`
  namespace.
- Do not probe cloud APIs at startup and do not send a chat request as a probe.
- Preserve Issue 43/45 REPL rendering behavior and Issue 44's documented PTY
  launcher by incorporating their committed predecessor changes before editing
  overlapping files.

## Tests

- Unit-test probe eligibility, one-request model detection, stopped-host and
  missing-model warnings, and fixed provider error guidance for connection,
  404, and authentication classes.
- Unit-test missing-key remediation and the banner/UX-demo guidance line.
- Extend the opt-in PTY suite to prove an unreachable Ollama warning includes
  the configured host and remediation while the `commandagent>` prompt remains
  usable; also cover the missing-model startup warning.
- Run focused provider, banner, UX-demo, config, and PTY checks first, then the
  repository-required format, Clippy, and full Rust test suite.
