# Issue #215 Design

## Scope and predecessor

Implement the approved approval-message and startup-warning change in a new
leaf module under `src/tools/`, with only minimal wiring in the CLI entry point
and tool registry. Do not change README or other CLI presentation because the
approved row decision narrows the original Issue proposal to `src/tools/` and
focused headless coverage.

Required predecessor Issue #238 is complete at commit `5ef42c7f`. It adds a
per-registry repeated-read cache and touches the execution path in
`src/tools/registry.rs`, but it does not alter the approval guard. This Issue
will keep its registry change to the existing approval check so it can be
applied alongside the predecessor without changing repeated-read semantics.

## Approval presentation

Add `src/tools/approval.rs` as the owner of approval-facing text and policy
presentation. It will provide:

- a startup-warning helper that returns a warning only for direct `--prompt`
  actions when stdin is not a TTY and `--yes` is absent; and
- an approval-denial helper used by `ToolRegistry` when neither automatic nor
  interactive approval is available.

The headless warning will state before provider/tool execution that mutating
tools cannot be approved in this mode and that `--yes` is available only for a
trusted workspace. The denial will retain the stable `approval required for
<tool>` classification prefix while replacing the impossible "use interactive
approval" suggestion with the executable `--yes` rerun choice and its trust
qualification.

The warning is intentionally limited to `Action::Prompt`: TTY prompt runs,
`--yes` runs, REPL, planning commands, and unrelated actions retain their
current presentation.

## Tests and verification

Add leaf tests covering the headless warning predicate and denial wording,
including negative cases for TTY, `--yes`, and non-prompt actions. Add a
focused process-level headless test that runs `--prompt` with piped stdio and
proves the warning is emitted before execution, plus a `--yes` case proving it
is suppressed.

Run the focused leaf and integration tests first. Because this changes shared
tool and direct-CLI behavior, then run formatting, Clippy across all targets,
and the full Rust test suite. No corpus fixture is needed because no event,
recovery, or corpus contract changes.
