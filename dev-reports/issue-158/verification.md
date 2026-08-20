# Issue 158 verification

- Status: `blocked`

## Checks

- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke -- --gate-one-only --output /private/tmp/commandagent-issue158-gate-one-rebuilt --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue158-full-rebuilt --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `failed`
- `git diff --check`: `passed`

## Candidate identity

The release binary was rebuilt after cherry-picking Issue 162 and reported:

```text
commandagent 0.1.0 64a52d1c+dirty 2026-08-21T02:58:03+09:00
```

## Passing Gate 1 evidence

`/private/tmp/commandagent-issue158-gate-one-rebuilt/browser-smoke.json`
reports overall `ok: true` for `/` and `/proxy/commandagent/`. For both cases:

- at 1440px, the markdown card reports `scrollWidth=385` and
  `clientWidth=385`; the confirmation ID reports `357` and `357`;
- at 390px, the markdown card reports `scrollWidth=316` and
  `clientWidth=316`; the confirmation ID reports `288` and `288`;
- all four layout records have `scroll_width_within_client: true`, and each
  markdown card contains four full SHA-256 values.

The same directory contains the fresh desktop and mobile Gate 1 screenshots
for both base paths.

## Definitive full-smoke failure

`/private/tmp/commandagent-issue158-full-rebuilt/browser-smoke.json` reports
overall `ok: false`. The root case timed out after 30 seconds at
`gui/scripts/smoke.mjs:833` while clicking the second
`[data-testid='reconnect-session-button']`. Playwright repeatedly observed the
resolved button as disabled. Page diagnostics were:

```text
page_error=null
stage=下書き
url=http://127.0.0.1:65535/try/?session=01a02058-956b-7f21-8ed5-d5c13abcf4c8
```

The harness stops after the first failed case, so the proxy full-smoke case was
not executed. This required failure blocks overall verification even though
the focused Issue 158 acceptance assertions pass.

## Stale-binary diagnostics

Two earlier failed runs are preserved but are not used for final verification:

- `/private/tmp/commandagent-issue158-smoke` predates Issue 162 integration;
- `/private/tmp/commandagent-issue158-full-integrated` had Issue 162 source in
  the worktree but used the pre-cherry-pick release binary.
