# Issue #67 Design

## Context and predecessor review

The Trial form currently embeds four profile options, four provider options, a
demo Goal, and `qwen3:8b` for both model roles. Changing only the provider can
therefore leave an incompatible model pin that Gate 1 faithfully freezes and
whose failure appears only after CLI delegation.

Issues 63 (`4313d7ef`), 64 (`7fcb0dbe`), and 66 (`d6f0dec5`) are complete on
parallel, non-ancestor branches from this worktree's base. Their committed
changes respectively cover polling recovery, workspace-lease recovery, and
post-run lifecycle locking/reset. They do not define Trial option discovery or
model guidance. This patch will not merge those independent histories; its
overlapping page, smoke, documentation, and guard-test edits will stay local to
initial values, option rendering, and preflight guidance so the lifecycle work
can be integrated normally.

## Design

- Add a read-only `GET api/trial-options` handler in a new GUI-server leaf
  module. It will enumerate profiles from `admitted_profiles()` and expose the
  four already-admitted CLI providers with display labels and provider-specific
  model hints. The handler performs no provider call, filesystem mutation, or
  Trial execution authorization because it returns only compiled product
  metadata needed before a token can be entered.
- Make the provider list in that module the single GUI-server source used by
  both the response and `SessionSpec` validation. This prevents the displayed
  provider choices from drifting from Gate 1 admission without widening the
  shared CLI configuration API.
- Replace hard-coded `<option>` elements with the fetched response. Show the
  selected profile's explanation, and after a provider change show an explicit
  warning that model pins are not rewritten plus the selected provider's model
  identifier guidance.
- Keep the existing `python-cli` and `ollama` selections, but initialize Goal,
  executor model, and planner model as empty strings. Check Goal, executor
  model, and planner model locally before sending a proposal; Goal validation
  runs before token validation so an untouched form gives direct Goal guidance
  rather than depending on the server's 422 response.
- Give the two model controls stable test IDs and have `smoke.mjs` explicitly
  fill Goal and both model roles. The real smoke continues to use its explicit
  `--model` input and is not coupled to browser defaults.

## Tests and verification

- Add a Rust integration test that calls the options endpoint without Trial
  credentials and compares its returned profile IDs exactly with
  `admitted_profiles()`; also pin the provider IDs and non-empty guidance.
- Update the GUI read-only source guard to require server option fetching,
  empty demo fields, local validation, dynamic option rendering, provider
  guidance, and explicit smoke fills while retaining the no-provider-call
  audit.
- Run the focused GUI server/read-only tests, GUI lint/typecheck/build and
  smoke-script syntax check, then formatting, Clippy across all targets, and
  the full Rust test suite because the binary routing and shared integration
  test surfaces change.

No event name or schema, corpus contract, historical evidence, or `.anvil/`
runtime namespace changes are required.
