# Issues #172 and #167 verification

- Status: `passed`

## Checks

- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `node --check gui/scripts/smoke.mjs`: `passed`
- `node --check gui/scripts/session-index-smoke.mjs`: `passed`
- `cargo test --test gui_read_only_guard trial_feedback_restores_sessions_and_uses_an_honest_terminal_title`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cd gui && npm run smoke -- --feedback-only --output /tmp/commandagent-issue-172-feedback-smoke --commandagent-bin ../target/debug/commandagent`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-172-session-index-smoke`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-172-full-smoke-rerun --commandagent-bin ../target/release/commandagent`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Evidence notes

- The focused feedback smoke passed for the root and `/commandagent/` base paths.
  It observed automatic restoration after reload, removal of `sample`, retention
  of the edited user goal, GET-only session requests, and the exact failing title
  `✗ すべての必須チェックには合格していません | CommandAgent`.
- The session-index smoke passed for both base paths and observed automatic,
  GET-only restoration from the history row and runtime header links.
- The authoritative full browser smoke passed for both base paths with the
  release binary. An initial sandboxed smoke attempt could not bind its loopback
  server; it was rerun in the approved environment. During iteration, the smoke
  was updated for the new automatic lifecycle and the final complete rerun passed.
