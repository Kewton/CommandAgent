# Issue #450 implementation

Added a reusable, opt-in evaluation Browser preflight and guarded campaign launch.
The CLI is `scripts/browser-preflight.py`; usage and request examples are in
`scripts/browser_preflight/README.md`. All changes are new task-owned files.

## Behavior

- Exact current-session skill/entry provenance and file checks; installed versions
  are inventory only. Unknown provenance and missing historical paths remain
  separate from files that exist but cannot operate a browser.
- A locked, durable ledger reserves work before browser actions. Fresh successful
  evidence is reused. An outstanding attempt prevents duplicates; two unchanged
  failed attempts suppress further retries. Changed conditions/recovery evidence
  require a saved reason and retain both condition fingerprints.
- The plugin adapter accepts the existing Browser handle, works with an empty tab
  list, performs one declared harmless action and closes only its own tab. The
  authorized Playwright fallback launches only an isolated temporary profile and
  cleans it up on success and operation failure.
- Validation requires the exact target URL, readable before/after state, the
  declared action's visible effect and a decodable, hashed image. HTTP-only success
  and incomplete environment metadata cannot freeze a campaign.
- Immutable manifests pin the verified method and observed browser/automation
  versions, headless mode, viewport, locale and timezone. The start wrapper checks
  evidence age (15 minutes), latest session outcome and hashes before running an
  explicitly supplied command. Starts are reserved once; changes need a new
  campaign and fresh preflight.

## Acceptance evidence

The 12 diagnostic fixtures and 47 Python tests cover the requested failure cases,
safe reuse, suppression after process restart, reasons for changed conditions,
fallback authorization/cleanup, malformed metadata, artifact tampering and denied
command execution. Four Node tests exercise the plugin adapter without bootstrap
or reinitialization.

Local GUI smoke used a task-owned loopback HTML fixture, not model generation or
the integration worktree's generated application. Both browser methods verified
URL, readable state, harmless button effect and screenshot capture. Saved evidence
is under `smoke/`; mutable local ledger/lock/start files are ignored and unstaged.

- Isolated profile: Chrome **152.0.7977.77**, Python Playwright **1.54.0**, headed,
  **1000 × 800**, **en-US**, **Asia/Tokyo**. Campaign freeze, rejection of a changed
  target, healthy reuse and a harmless guarded Python launch succeeded. Its PNG
  was decoded and visually inspected. Browser/profile and server were closed.
- Current plugin **26.825.51511**: initial connection worked. The adapter reused
  that binding after an empty tab list and captured the changed page. Its image
  is actually JPEG (as recorded in the artifact metadata; the initial smoke file
  had a `.png` suffix). The adapter now preserves JPEG suffixes for future captures.
  The supported page scope did not expose navigator/environment metadata, so
  `conditions_unverified` correctly blocked freezing this method. This is an
  expected refusal, not a failed GUI operation. Only task-owned tabs/server closed.

## State and limits

Automatic diagnosis and repeated-failure suppression: **implemented**.
Temporary recovery/independent-profile operation: **verified locally**.
Plugin-internal permanent repair: **not implemented or established**.
Plugin update/resume/reinitialize lifecycle recurrence tests: **not established**.

Current-session instruction identity and adapter receipts are trusted evaluator
inputs, not cryptographic attestations. The guard applies to commands launched
through the new wrapper and does not instrument later arbitrary browser actions.
The frozen 0908 harness, plugin cache, live runtime, integration worktree evidence
and unrelated product/README/CHANGELOG/UX files were not changed. Rust/event/corpus
contracts are unaffected.

Focused checks and GUI smoke pass. The broad existing Python run has four failures
in unchanged tests/prerequisites; verification is therefore recorded as `blocked`
and detailed in `verification.md`. Their hashes match the parent commit in
`regression-findings.json`; no acceptance gates or historical evidence were weakened.
