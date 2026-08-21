# Issue 158 verification

- Status: `passed`

## Checks

- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke -- --gate-one-only --output /private/tmp/commandagent-issue158-resume-gate-one --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cd gui && npm run smoke:session-index -- --output /private/tmp/commandagent-issue158-resume-session-index`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue158-resume-full --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && node --check scripts/smoke.mjs && node --check scripts/session-index-smoke.mjs && node --check scripts/storage-smoke.mjs && node --check scripts/error-smoke.mjs`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui`: `passed`
- `git diff --check`: `passed`

## Candidate and evidence

The release binary was rebuilt after integrating `ea8f8fbd` and reported:

```text
commandagent 0.1.0 ceaa6d36 2026-08-21T10:14:45+09:00
```

The focused Gate 1 evidence is
`/private/tmp/commandagent-issue158-resume-gate-one/browser-smoke.json`, the
wrong-token retry evidence is
`/private/tmp/commandagent-issue158-resume-session-index/session-index-smoke.json`,
and the unchanged full-smoke evidence is
`/private/tmp/commandagent-issue158-resume-full/browser-smoke.json`. Each report
has overall, root, and proxy `ok: true`.

At 1440px the markdown and confirmation hash surfaces report 385/385 and
357/357 `scrollWidth/clientWidth`; at 390px they report 316/316 and 288/288.
The full smoke also records rejected-token removal, GET-only reconnects,
preserved elapsed/mean timing, no unexpected console errors, and the expected
launch-control lifecycle for both base paths.

## Superseded diagnostics

These earlier failures are retained as diagnostics but do not determine the
rebuilt follow-up candidate verdict:

- `/private/tmp/commandagent-issue158-smoke` predates Issue 162 integration;
- `/private/tmp/commandagent-issue158-full-integrated` used a pre-cherry-pick
  release binary despite integrated source;
- `/private/tmp/commandagent-issue158-full-rebuilt` exercised `64a52d1c` before
  the verified auth-retry follow-up and exposed the reconnect race fixed by
  `ea8f8fbd`.

## CI follow-up checks (`b6eb6642`)

- `cargo fmt --all -- --check`: `passed`
- `cargo +1.97.1 clippy --features gui --bin gui_server -- -D warnings`: `passed`
- `cargo test --features gui --test gui_server trial_session_files_`: `passed`
- `cargo test --features gui --test gui_server gui_server_read_errors_use_the_shared_coded_json_contract`: `passed`
- `cd gui && npm run smoke -- --gate-one-only --output /private/tmp/commandagent-issue158-b6eb6642-gate-one-escalated --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`

The two `trial_session_files_` cases passed for authenticated access, bounded
listing/tails, exact successful event bytes, traversal rejection, oversized
and invalid request status/code behavior, and file, run-root, and runtime-root
symlink rejection. The shared coded-JSON test also passed. Code inspection
confirms `SessionFileError` returns the same boxed `Response`, preserving its
status, headers, and JSON bytes without reconstruction.

`/private/tmp/commandagent-issue158-b6eb6642-gate-one-escalated/browser-smoke.json`
reports overall, root, and proxy `ok: true`. At 1440px the markdown and
confirmation surfaces remain 385/385 and 357/357
`scrollWidth/clientWidth`; at 390px they remain 316/316 and 288/288. All four
surfaces have `scroll_width_within_client: true`, and there are no unexpected
console errors.

The first sandboxed smoke attempt at
`/private/tmp/commandagent-issue158-b6eb6642-gate-one` could not bind
`127.0.0.1:0` (`Operation not permitted`) and did not reach browser assertions.
The authorized loopback rerun above is the verification result.
