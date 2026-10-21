# Issue #450 implementation

Added a reusable, opt-in evaluation Browser preflight and guarded campaign launch.
The CLI is `scripts/browser-preflight.py`; usage and request examples are in
`scripts/browser_preflight/README.md`. Browser implementation and evidence use new
task-owned files. A subsequently authorized, separate maintenance change modifies
only the existing retrospective test and associated reports.

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
- V2 requires an explicit `required_viewports` list. Each configured viewport needs
  its own measured size, post-action page state and decoded image with matching
  dimensions. Missing/mismatched second images block freeze and launch. The entire
  viewport list is pinned; historical v1 single-view reports cannot authorize v2.
- Immutable manifests pin the verified method and observed browser/automation
  versions, headless mode, viewport, locale and timezone. The start wrapper checks
  evidence age (15 minutes), latest session outcome and hashes before running an
  explicitly supplied command. Starts are reserved once; changes need a new
  campaign and fresh preflight.

## Acceptance evidence

The 12 diagnostic fixtures, explicit evaluation viewport fixture and 64 Python tests cover the requested failure cases,
safe reuse, suppression after process restart, reasons for changed conditions,
fallback authorization/cleanup, malformed metadata, artifact tampering and denied
command execution. Five Node tests exercise the v2 plugin adapter without bootstrap
or connection reinitialization, including unsupported viewport control and cleanup.

Local GUI smoke used a task-owned loopback HTML fixture, not model generation or
the integration worktree's generated application. Both browser methods verified
URL, readable state, harmless button effect and screenshot capture. Saved evidence
for the complete two-viewport requirement is under `smoke-viewports/`; mutable
local ledger/lock/start files are ignored and unstaged. The initial single-view
v1 evidence under `smoke/` remains historical and does not satisfy that requirement.

- Isolated profile: Chrome **152.0.7977.77**, Python Playwright **1.54.0**, headed,
  configured **desktop 1440 × 900** and **mobile 390 × 844**, **en-US**,
  **Asia/Tokyo**. Both `innerWidth/innerHeight` measurements and independently
  decoded PNG dimensions match the configuration. Campaign freeze, rejection of a changed
  target, healthy reuse and a harmless guarded Python launch succeeded. Its PNG
  images were decoded and visually inspected. Browser/profile and server were closed.
- Current plugin **26.825.51511**: initial connection worked. The adapter reused
  that binding after an empty tab list, applied the documented viewport capability
  and captured both views. Both measured viewports and JPEG dimensions match
  1440 × 900 / 390 × 844. The temporary viewport override was reset afterward.
  The supported page scope did not expose navigator/environment metadata, so
  `conditions_unverified` correctly blocked freezing this method. This is an
  expected refusal, not a failed GUI operation. Only task-owned tabs/server closed.

The v2 smoke initially encountered a local helper import failure because the Node
tool did not accept a query-suffixed module path. That error was recorded before
any browser operation. The helper now has the explicit `plugin_probe_v2.mjs`
filename; one retry with the changed-path reason saved in the ledger completed
both views. The healthy Browser connection was retained throughout. The parent
conversation's separate old-service resolution failure was read as reference
evidence only; it was not generalized to this worker or treated as repaired.

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

Focused checks and GUI smoke pass. The original broad run had four failures and
remained `blocked` through viewport commit `9b313c51`; its original observations
remain in `regression-findings.json`. After the parent restored exact-hash ignored
prerequisites and built the missing binaries, the user authorized a separate,
test-only repair for the stale retrospective inventory assertion.

Source commit `d39c84a3368d8b59ca0d450b439ad32133e8b78e` introduced the six Luna
ingest rows. The repaired test preserves the frozen 287 rows' original profile
counts/full=10 assertions, additionally compares every historical row dictionary,
requires exactly the six new Luna IDs/full results, and asserts current
293 rows / ingest 60 / full 16. Score 100 and all-atoms-pass checks remain applied
to every full row. No production scanner or historical input was edited.

The focused retrospective module passed 11 tests and 6 subtests; Ruff passed. The
unchanged full broad command then passed **571 tests**, with **11 optional skips**
and **79 subtests passed**, under **Python 3.12.3** with `login=false`.
`verification.md` records the original failure and final successful rerun honestly;
the final status is `passed`. See `gate-repair-design.md` and
`gate-repair-integrity.json` for the separate maintenance rationale and integrity
checks. No acceptance gate was weakened or historical record rewritten.
