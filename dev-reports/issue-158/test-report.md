# Issue 158 test report

- Status: `passed`

## Release candidate

`cargo build --release --bin commandagent` rebuilt the candidate after the
Issue 162 follow-up integration. `target/release/commandagent --version`
reported `commandagent 0.1.0 ceaa6d36 2026-08-21T10:14:45+09:00`.

## Browser acceptance

The focused Gate 1 root/proxy smoke passed at the exact 1440px and 390px
viewports. In both base-path cases:

- the markdown surface contained four SHA-256 values and measured 385/385 at
  1440px and 316/316 at 390px (`scrollWidth/clientWidth`);
- the separate confirmation hash measured 357/357 and 288/288;
- every `scroll_width_within_client` assertion was true.

Evidence: `/private/tmp/commandagent-issue158-resume-gate-one/browser-smoke.json`.

The session-index wrong-token smoke passed for root and proxy. Both cases
record `rejected_token_removed`, `retry_button_enabled`, and
`reconnect_get_only` as true while ordinary focus and visibility revalidation
remain active. Evidence:
`/private/tmp/commandagent-issue158-resume-session-index/session-index-smoke.json`.

The unchanged full root/proxy smoke passed with `qwen3:8b`. Both cases retain
the Gate 1 width results, remove a rejected token, use GET-only reconnects,
preserve elapsed and measured-mean values after reconnect, reach a second
terminal session, and record no unexpected console errors. Evidence:
`/private/tmp/commandagent-issue158-resume-full/browser-smoke.json`.

## Contract audit

During compose/reconnect, `TrialRun` sets `deferAutomaticRevalidation` when a
reconnect session ID is present. `TrialSessionIndexPanel` then invalidates
stale request generations and suppresses initial, focus/visibility,
revision-key, and runtime-lease automatic revalidation; explicit refresh still
calls the manual retry path.

The unchanged full smoke's launch-control locator covers exactly seven fields:
goal, token, profile, pack, provider, executor model, and planner model. It
requires that locator to resolve zero controls at Gate 2, terminal, and closed
states, then requires all seven controls to be enabled after starting a new
run. Both root and proxy lifecycle records passed these assertions.

## Static and Rust checks

GUI lint, TypeScript checking, all four smoke-script syntax checks, and the
production Next.js build passed. Rust formatting, Clippy, and tests passed for
both default and `gui` feature configurations.
