# Issues #190, #191, and #192 verification

- Status: `passed`

## Checks

- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `node --check gui/scripts/smoke.mjs`: `passed`
- `node --check gui/scripts/session-index-smoke.mjs`: `passed`
- `cargo test --test gui_read_only_guard gui_language_navigation_titles_and_runtime_status_are_pinned`: `passed`
- `cargo test --test gui_read_only_guard gui_visibility_revalidation_and_shared_time_format_are_pinned`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-190-overview-smoke --commandagent-bin ../target/debug/commandagent`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-190-session-index-smoke`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test --lib fetch_probe::transport::tests::bounded_child_transport_uses_a_config_path_not_a_url_argument -- --exact`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Evidence notes

- The overview smoke passed at `/` and `/proxy/commandagent/`. All five primary
  routes retained the in-document marker across navigation, rendered the expected
  base-path-prefixed href, and exposed exactly one `aria-current="page"` item.
- The session-index smoke passed at both base paths. The synthetic terminal
  runtime refresh completed in 283 ms at `/` and 179 ms at
  `/proxy/commandagent/`; both stayed below one second, and maximum concurrent
  runtime requests remained one.
- The first sandboxed overview-smoke attempt could not bind its loopback server
  (`Operation not permitted`). The approved outside-sandbox rerun passed.
- An initial full `cargo test` run had one unrelated transient `fetch_timeout` in
  `bounded_child_transport_uses_a_config_path_not_a_url_argument` while 2,004
  library tests passed. The exact test then passed, and the subsequent complete
  `cargo test` rerun passed.
