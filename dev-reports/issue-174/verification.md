# Issues 174, 175, 176, 179, 180, 196, 197, 198, and 202 verification

- Status: `passed`

## Checks

- `cargo test --features gui --bin gui_server sessions::tests -- --test-threads=1`: `passed`
- `cargo test --test gui_read_only_guard -- --test-threads=1`: `passed`
- `npm run lint` (from `gui/`): `passed`
- `npm run typecheck` (from `gui/`): `passed`
- `npm run build` (from `gui/`): `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test --features gui -- --test-threads=1`: `passed`

## UAT repair loop 1

- Original failure: candidate `94379e4bab6e76e569f90802f337a8be04150e73`
  failed the root `probeTenMinutePolling` probe at
  `gui/scripts/smoke.mjs:2531`. The harness waited for the visible raw value
  `running`, while the approved localized UI contract displays `実行中`.
- Repair scope: update only that visible UI expectation. API and wire fixtures
  continue to use the raw status value `running`, and smoke control flow is
  unchanged.
- `node --check gui/scripts/smoke.mjs`: `passed`
- `npm run smoke -- --output /private/tmp/commandagent-issue-174-polling.OobGLK --polling-only`
  (from `gui/`, outside the sandbox): `passed` for root and proxy.
- `npm run smoke -- --output /private/tmp/commandagent-issue-174-full-smoke.lHOy6V`
  (from `gui/`, outside the sandbox): `failed` in the root
  `probeSessionIndexLease` probe at `gui/scripts/smoke.mjs:2415`; the probe
  attempted to click `check-contract` after routing the workspace lease as
  `running`, but the approved Issue 179 behavior keeps that control disabled.
  The smoke runner stopped after the root failure, so the proxy full-smoke case
  did not run.
- Initial repair result: the authorized polling expectation is fixed and its focused
  root/proxy probe passes, but the required full smoke remains blocked by a
  separate stale control-flow expectation outside this repair's edit scope.

### Scope amendment for the second UAT failure

- The second original failure is the full-smoke root
  `probeSessionIndexLease` failure recorded above: the probe encoded the
  pre-Issue 179 flow by clicking `check-contract` and entering Gate 1 even
  though its routed lease was `running`.
- Amended repair: keep the existing session-index, reconnect, Japanese-status,
  and admitted-pack assertions; assert that `check-contract` is disabled before
  proposal, that `lease-inline-notice` contains the session ID and
  `新しい起動はできません`, and that proposal and dispatch POST counts both
  remain zero. Product code and raw API/wire values remain unchanged.
- `node --check gui/scripts/smoke.mjs`: `passed`
- Focused probe mode: not available. The smoke mode dispatcher has no flag that
  invokes `probeSessionIndexLease` independently; the full mode is the narrowest
  available exercising mode.
- First amended full-smoke attempt:
  `npm run smoke -- --output /private/tmp/commandagent-issue-174-amended-full-smoke.M1PlxZ`
  (from `gui/`, outside the sandbox): `failed` after the amended probe because
  the required default delegate binary `target/release/commandagent` was absent.
- `cargo build --release --bin commandagent`: `passed`; this supplied the smoke
  prerequisite without changing source.
- Required full-smoke rerun:
  `npm run smoke -- --output /private/tmp/commandagent-issue-174-amended-full-smoke-rerun.jpdNQ2`
  (from `gui/`, outside the sandbox): `failed` later in the root case at
  `gui/scripts/smoke.mjs:1081`. The wait compares visible session-index text to
  the raw API gate value from `finalApi.body.gate`, while the approved localized
  UI displays the Japanese gate label. The runner stopped after the root
  failure, so the proxy case did not run.
- Amended repair result: the run advances past `probeSessionIndexLease`, but the
  required full root/proxy smoke remains blocked by another stale raw-value UI
  expectation outside the authorized probe. No commit was created.

### Scope amendment for the final gate-label wait

- The next root full-smoke failure occurred at `gui/scripts/smoke.mjs:1081`:
  the session-index wait searched visible text for the raw API gate value
  `gate_3` or `gate_4`, while the same case expects `GATE 3（完了）` or
  `GATE 4（要対応）` in its final assertion. The wire-value assertions remained
  valid and unchanged.
- Amended repair: compute one local expected Japanese gate label and reuse it in
  both the session-index wait and the existing visible-text assertion.
- `node --check gui/scripts/smoke.mjs`: `passed`
- Final required full smoke:
  `npm run smoke -- --output /private/tmp/commandagent-issue-174-final-full-smoke.OzNlwu`
  (from `gui/`, outside the sandbox): `failed` in the root case at the amended
  session-index wait, now `gui/scripts/smoke.mjs:1084`. The expected localized
  gate label was not observed before the 30-second timeout. The runner stopped
  after the root failure, so the proxy case did not run.
- Final result: `blocked`. Per the scope-amendment stop condition, no further
  change or rerun was attempted and no commit was created.

## UAT repair loop 2

- Diagnosed failure: `.session-status` applies `text-transform: uppercase`, so
  the line-1084 wait read DOM source text `Gate 3（完了）` through
  `textContent`, while the later user-visible assertion read
  `GATE 3（完了）` through `innerText`.
- Repair: read each row's visible `innerText` in the wait. The shared
  `expectedFinalGateLabel`, raw `finalApi` wire assertions, and both prior
  strict harness repairs remain unchanged.
- `node --check gui/scripts/smoke.mjs`: `passed`
- Full root/proxy smoke:
  `npm run smoke -- --output /private/tmp/commandagent-issue-174-loop2-full-smoke.CUL2nx`
  (from `gui/`, outside the sandbox): `failed` after completing both root and
  proxy cases. The two repaired probes passed in both cases, including
  `session_index_lease.ok: true`, but each case reported
  `dashboard.trial_compose_regression.incompatible_pack_normalized: false`.
  That separate check still expects the pre-localization warning fragment
  `現在の profile / intent では選べません`, while the visible UI reports
  `このパックは現在のプロファイル / 目的では選べません。`.
- Repair result: `blocked`. Per the loop-2 stop condition, no further harness
  edit or rerun was attempted and no commit was created.

## UAT repair loop 3

- Diagnosed failure: the completed loop-2 root and proxy cases failed only
  because the incompatible-pack normalization check expected the stale visible
  fragment `現在の profile / intent では選べません` instead of the captured,
  approved Japanese warning
  `このパックは現在のプロファイル / 目的では選べません。`.
- Repair: replace only that warning expectation. The selected value, profile,
  proposal body, response status, raw wire assertions, and all prior strict
  repairs remain unchanged.
- `node --check gui/scripts/smoke.mjs`: `passed`
- Final full root/proxy smoke:
  `npm run smoke -- --output /private/tmp/commandagent-issue-174-loop3-final-smoke.BCl3jb`
  (from `gui/`, outside the sandbox): `failed` after completing both cases.
  The loop-3 normalization check and prior strict lease repairs passed in root
  and proxy (`trial_compose_regression.ok: true` and
  `session_index_lease.ok: true`). Both cases still reported aggregate
  `ok: false` because the conflict-reconnect check derives
  `conflictReconnectId` from an anchor `href`; the approved reconnect control is
  now a native button, so that value is `null` even though the session query is
  correctly updated and the reconnect guidance is present.
- Repair result: `blocked`. No further repair loop is authorized, so no further
  harness edit or rerun was attempted and no commit was created.

## UAT repair loop 4

- Additional authorized scope: repair two confirmed integration-harness
  assumptions without changing product code.
- Gate 1 copy repair: retain every required Japanese contract-card assertion
  and the `MEASURED PRICE TAG` exclusion, but require the approved visible
  heading `GATE 1 / 見積り` instead of removed copy.
- Conflict-reconnect repair: replace the obsolete anchor-`href` ID derivation
  with a native-button contract for `reconnect-session-link`. Require a visible
  `BUTTON` with `type="button"` and an accessible/visible exact name containing
  the session ID and `再接続`. Retain reconnect guidance, matching session
  query, intercepted dispatch count, GET-only lifecycle, and every other
  aggregate assertion.
- `node --check gui/scripts/smoke.mjs`: `passed`
- `npm run smoke -- --output /private/tmp/commandagent-issue-174-loop4-full-smoke.pHRXt5`
  (from `gui/`, outside the sandbox): `passed`; the report records aggregate
  `ok: true` with both root and proxy cases `ok: true`.
- Repair result: `passed`. All prior failure and repair history remains above.

## CI ownership correction

- PR head `cb2a5c8423876bca638b6a5600cd14accc37176e` failed CommandAgent Test,
  Guardrails, and acceptance on the same foundation-owned assertion in
  `tests/gui_read_only_guard.rs:1792`, which requires the pre-existing
  `launch_disabled: launchDisabled` smoke source contract.
- Ownership decision: do not edit the foundation-row guard and do not change
  product code. The successful loop-4 root/proxy smoke result remains valid
  external/local UAT evidence, but its harness overlay is intentionally not
  shipped in this Trial PR because `gui/scripts/smoke.mjs` integration-harness
  ownership belongs to the #176/foundation row.
- Restoration: `gui/scripts/smoke.mjs` was restored with explicit patch edits
  to the exact candidate `94379e4bab6e76e569f90802f337a8be04150e73`
  content. Both files hash to Git blob
  `4e0a6de4310ec720cfc665740bea4196c671b71d`, and a path-scoped `git diff
  --exit-code` reports no difference.
- `node --check gui/scripts/smoke.mjs`: `passed`
- `cargo test --test gui_read_only_guard trial_session_index_is_bounded_read_only_and_reconnects_by_link -- --exact`:
  `passed` (1 passed, 24 filtered out).
- Ownership-correction result: `passed`. The shipped harness is byte-identical
  to candidate `94379e4b`, its owned guard passes, and the successful loop-4
  external/local UAT evidence and complete repair history remain recorded.
