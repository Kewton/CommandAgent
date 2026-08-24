# Issues #177, #223, and #216 Design

## Scope and constraints

- Keep `ConfirmationIdentity` and `ConfirmationIdentity::card_hash()` unchanged.
  The GUI and CLI will render the same canonical identity and confirmation ID.
- Keep `render_gate_one` as the CLI entry point so its `/confirm <hash>` guidance
  remains backward compatible. Add a GUI entry point that changes presentation
  only: it omits the CLI command and explains the internal `profile` preset as
  the selected profile's default.
- Keep the existing typed Gate 4 action set and availability decisions. Add one
  concrete operation to every rendered action, using existing supported flows:
  re-enter the request for `retry`, `/resume` for recovery, restart with a
  stronger model for `elevated_model`, `/pack`, `/directive`, and `/exit` for
  `close`.
- Keep provider telemetry's `caller_scope` and timeout behavior unchanged. The
  route classifier remains a `planner_step` internally, but its classifier-only
  provider-call override supplies a user-facing `classifier` display scope.
  Footer and breadcrumb leaf renderers translate that scope to request
  classification wording before Gate 1.
- Do not change event names, event schemas, `.anvil/` state, acceptance rules,
  or persisted historical evidence.

## Implementation seams

1. Refactor Gate 1 line construction in
   `src/tui/boundary_shell/presentation.rs` behind a private surface enum and
   expose `render_gate_one_for_gui` alongside the unchanged CLI function.
2. Update the GUI Gate 1 leaf handler to call the GUI renderer.
3. Centralize action-operation copy in the boundary presentation so every
   typed action line contains an operation regardless of its availability.
4. Extend the classifier-specific provider-call override with a display scope;
   use it only for status/breadcrumb presentation while retaining the canonical
   telemetry scope.

## Tests and verification

- Add focused presentation tests comparing GUI and CLI Gate 1 output, pinning
  the shared hash, GUI-only omissions/explanation, and unchanged CLI guidance.
- Update Gate 4 presentation tests to require concrete operations for all typed
  actions.
- Strengthen the GUI server proposal regression so its returned markdown cannot
  expose `/confirm` or the unexplained `計画プリセット: profile` text.
- Add an ignored, opt-in PTY regression with a delayed fake classifier. It must
  reach Gate 1 with classification wording and no `planning` text before the
  card.
- Run focused Rust tests first, the opt-in PTY regression, then formatting,
  Clippy, and the full Rust suite because shared TUI/provider presentation is
  touched.
