# Issue 46 implementation summary

## Implementation

- Added `providers::startup`, a leaf module that produces warning-only startup
  diagnostics. `warnings` returns nothing unless the run is an interactive
  `Action::Repl` with a TTY and `--offline` unset. It probes only when a
  configured executor or planner role uses Ollama, de-duplicating repeated model
  names through a `BTreeSet`. It performs exactly one `GET /api/tags` with a
  two-second connect and request timeout, then reports either host
  unreachability or each configured model missing from the installed set.
- Wired the diagnostics into `tui::repl::run`: the warnings print with
  `eprintln!` after the startup banner and before the normal provider clients are
  constructed, and the REPL always continues to the editor. No new event is
  emitted, so existing event schemas are unchanged.
- Added `providers::guidance`, a leaf module with `connection_error` and
  `http_status_error` helpers that append a fixed `Hint:` line. Connection
  failures, HTTP 404, and HTTP 401/403 receive remediation tailored to
  reachability, model setup (`ollama pull` for Ollama, access checks for cloud),
  or API-key setup. The original error text is preserved; control characters are
  neutralized to `?`. The Ollama, OpenAI, and Gemini blocking and streaming
  request paths all route their terminal failures through these helpers.
- Made `config::load_api_key` name the environment variable, the workspace
  `.env`, and `commandagent --doctor` in its missing-key error.
- Added one banner guidance line, `help: /help for commands | /doctor for setup
  diagnostics`, in `tui::banner::render_startup_banner`. Because `--ux-demo`
  renders the shared banner, the `tui::ux_demo` scripted-output assertion and the
  committed `docs/assets/ux-demo.svg` excerpt (new help line plus shifted text
  coordinates) were updated to match.

## Compatibility and documentation

No configuration, event names, event fields, or `.anvil/` namespace changed. The
startup probe never gates the prompt, never contacts cloud APIs, and never sends
a chat request as a probe. The Issue 43/45 REPL rendering path and Issue 44's
documented PTY launcher are preserved; the new PTY tests gate on
`COMMANDAGENT_PTY_TESTS`, which `env_compat` also resolves from the legacy
`ANVIL_PTY_TESTS` used by `just test-pty`.

## Tests

- Unit coverage in `providers::guidance` for the connection, 404, and 401/403
  guidance lines; in `providers::startup` for probe eligibility, one-request
  model detection, the missing-model and unreachable-host warnings, and the
  two-second probe bound; in `config` for the missing-key remediation; and in
  `tui::banner`/`tui::ux_demo` for the guidance line. `providers::ollama` gains
  runtime connection and 404 hint coverage.
- Integration coverage in `tests/provider_onboarding.rs` proving the OpenAI and
  Gemini missing-key command failures name the key, the environment, the
  workspace `.env`, and `commandagent --doctor`.
- Opt-in PTY coverage in `tests/tui_pty.rs` proving an unreachable Ollama warning
  carries the configured host and remediation while the `commandagent>` prompt
  stays usable, plus the missing-model startup warning.
- Stabilized the predecessor long-screen PTY scenario by allowing its lengthy
  `/status` redraw to finish before sending `/exit`; its assertions and product
  behavior are unchanged.
