- Status: `passed`

## Checks

- `cd gui && npm ci --include=dev`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && GUI_BASE_PATH=/ npm run build`: `passed`
- `cd gui && GUI_BASE_PATH=/proxy/commandagent/ npm run build`: `passed`
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-373-overview-smoke-final --commandagent-bin ../target/debug/commandagent`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-373-session-index-smoke`: `passed`
- `cargo test --test gui_read_only_guard -- --nocapture`: `passed`
- `cargo test --test doc_drift -- --nocapture`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Dependency CI-fix propagation

- Incoming Issue 370 head: `9e8e178b97b49c78411ad9d2ba1783168227cdd9`
- Incoming Issue 369 CI-race fix: `f0fb9ccfb6572f952c3a1b5d146d41b8b92eadac`
- Applied CI-race cherry-pick: `927f7ff3`
- Applied dependency-verification cherry-pick: `2a8d3891`
- `git diff --exit-code adbe8287 0a9d2a0a --`: `passed`
- `git diff --exit-code 31410760 032ed840 --`: `passed`
- `git diff --exit-code f0fb9ccf HEAD -- dev-reports/issue-369`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --test gui_read_only_guard`: `passed`
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-373-dependency-ci-fix-overview-smoke --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo clippy --all-targets -- -D warnings`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Merge recovery from origin/develop

### Base and ancestry

- Fetched base: `origin/develop` at `7b1c2d8df37053d8719d24ed18094a8a8c18012b`
- Pre-merge Issue 373 head: `ed6cdbbe80ca49329e97240a3d6ea42578f61197`
- Normal merge commit: `cd014064d72f3341b1d86340fb6cfd6831bbc82c`
- Merge first parent: `ed6cdbbe80ca49329e97240a3d6ea42578f61197`
- Merge second parent: `7b1c2d8df37053d8719d24ed18094a8a8c18012b`
- `git fetch origin develop`: `passed`
- `test "$(git rev-parse origin/develop)" = 7b1c2d8df37053d8719d24ed18094a8a8c18012b`: `passed`
- `git merge-base --is-ancestor 7b1c2d8df37053d8719d24ed18094a8a8c18012b cd014064d72f3341b1d86340fb6cfd6831bbc82c`: `passed`
- `git merge-base --is-ancestor ed6cdbbe80ca49329e97240a3d6ea42578f61197 cd014064d72f3341b1d86340fb6cfd6831bbc82c`: `passed`

### Conflict resolutions

The normal merge reported 11 textual conflicts. They were resolved as follows:

- `CHANGELOG.md` keeps the Issue 373 Overview entry together with develop's
  merged feature entries.
- `README.md` and `README.ja.md` keep the Issue 373 landing and Trial guidance
  while adopting Issue 372's bounded draft-profile registration wording.
- `dev-reports/issue-370/verification.md`, `docs/guide/README.md`,
  `docs/guide/en/extensions.md`, `docs/guide/ja/extensions.md`,
  `gui/app/assets/page.tsx`, and
  `gui/public/commandagent-gui-contract.json` match the exact develop versions,
  preserving merge-recovery evidence, the profile wizard, and contract v372.
- `gui/scripts/smoke.mjs` keeps the complete Issue 373 Overview, mobile,
  accessibility, and runtime-truth assertions while adding develop's profile
  wizard and supplied-profile coverage. The removed dismissible first-use
  assertion was not restored.
- `tests/gui_read_only_guard.rs` keeps the Issue 373 landing contracts and adds
  develop's profile-wizard contract assertion.
- `git diff --exit-code origin/develop -- workspace/management/runs docs/migration`: `passed`
- `git diff --exit-code ed6cdbbe80ca49329e97240a3d6ea42578f61197 cd014064d72f3341b1d86340fb6cfd6831bbc82c -- gui/app/page.tsx gui/components/getting-started.tsx`: `passed`

One semantic conflict was found by the session-index smoke after the textual
merge: develop's new resource-revalidation probe still selected run rows on
the old Overview dashboard. Issue 373 intentionally moved repository-run
detail to `/runs/`. The probe now checks the same retained-data and focus/
visibility revalidation contract through `#run-select` and `.run-picker` on
that owning page. It waits for native `<option>` elements to be attached,
without treating hidden option rendering as a failure.

- Initial `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-373-merge-recovery-session-index-smoke`: `failed` (`.runs-panel` no longer exists on Overview)
- Intermediate `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-373-merge-recovery-session-index-smoke-final`: `failed` (native `<option>` was attached but not visually rendered)
- Final `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-373-merge-recovery-session-index-smoke-final`: `passed`

### Post-merge checks

- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && node --check scripts/session-index-smoke.mjs`: `passed`
- `cd gui && GUI_BASE_PATH=/ npm run build`: `passed`
- `cd gui && GUI_BASE_PATH=/proxy/commandagent/ npm run build`: `passed`
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-373-merge-recovery-overview-smoke --commandagent-bin ../target/debug/commandagent`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --test gui_read_only_guard`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --test doc_drift`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo clippy --all-targets -- -D warnings`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

The final Overview report ended with `ok: true` for both `/` and
`/proxy/commandagent/`, zero Axe landing violations, responsive mobile fit,
and honest active/unavailable runtime projections. The final session-index
report ended with `ok: true` for both base paths, including lifecycle, source
matrix, failed-refresh retention, and focus-refresh replacement. Temporary
browser outputs remain under `/tmp`; no new run artifact was created.
