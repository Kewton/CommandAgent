# Issue #185 Verification

- Status: `passed`

## Checks

- `cargo test --test gui_read_only_guard measurement_filter_and_mobile_map_fit_are_pinned -- --exact`: `passed`
- `cargo test --test gui_read_only_guard run_detail_and_measurement_read_only_browsing_contracts_are_pinned -- --exact`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `node --check gui/scripts/smoke.mjs`: `passed`
- `npm run lint` (working directory: `gui`): `passed`
- `npm run typecheck` (working directory: `gui`): `passed`
- `npm run build` (working directory: `gui`): `passed`
- `cargo build --features gui --bin commandagent`: `passed`
- `npm run smoke -- --read-only --output /private/tmp/commandagent-issue185-smoke.7aAqYV --commandagent-bin ../target/debug/commandagent` (working directory: `gui`): `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Acceptance evidence

- `/private/tmp/commandagent-issue185-smoke.7aAqYV/browser-smoke.json` reports
  top-level and per-case `ok: true` for `/` and `/proxy/commandagent/`, with no
  unexpected console errors.
- Both cases exercised all 256 reports: exact-path filtering produced
  `1 / 256`, a missing query produced `0 / 256` and visible `該当なし`, and
  clearing restored `256 / 256` without changing the selected document.
- At 390px, both cases measured the map frame and image at 322×207. Horizontal
  and vertical overflow were both false, the image fit its frame, the frame fit
  one viewport, computed overflow was hidden on both axes, and the page fit the
  viewport width.
- The generated root Measurements mobile screenshot was visually inspected at
  original resolution and shows the full embedded SVG and report filter without
  map clipping or two-axis scrolling.
- `git diff --unified=0 -- gui/app/globals.css` shows that every stylesheet
  change is inside `.measure-map .map-frame` or
  `.measure-map .map-frame img`; no other global style changed.

## Setup note

This worktree initially had no `gui/node_modules`. `npm ci --include=dev
--offline` restored the exact lockfile-pinned dependencies from the local cache
without changing `package-lock.json`. The first sandboxed smoke attempt was
unable to bind `127.0.0.1:0`; the identical read-only command was rerun with
loopback/headless-browser permission and passed.
