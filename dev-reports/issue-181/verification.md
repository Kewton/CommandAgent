# Issue 181 verification

- Status: `passed`

## Checks

- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cargo test --test gui_read_only_guard gui_style_and_run_ledger_accessibility_contracts_are_pinned`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue-181-smoke.j9Fa3M --commandagent-bin ../target/release/commandagent`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `UV_CACHE_DIR=/private/tmp/commandagent-issue-181-uv-cache uv run --with pyyaml -- cargo test`: `passed`
- `git diff --check`: `passed`

## Evidence

- Smoke report:
  `/private/tmp/commandagent-issue-181-smoke.j9Fa3M/browser-smoke.json`
- Smoke result: both root and proxy-base-path cases reported `ok: true`.
- Mobile running header: 390px viewport, 60px top bar, two one-line badges,
  positive 15.796875px brand/summary gap, and viewport fit in both cases.
- Mobile close control: `line_count: 1`, `single_line: true`, and computed
  `white_space: nowrap` in both cases.
- Sticky-header clearance: computed 72px scroll margin against a 60px mobile
  top bar; execution and terminal headings cleared the header in both cases.
- Screenshots visually inspected: root and proxy
  `*-gate-2-mobile.png` and `*-getting-started-mobile.png`.

## Environment notes

The first direct full-suite attempt exposed a missing system-Python `yaml`
module. A focused rerun with temporary PyYAML passed. The first dependency-correct
full-suite rerun remained inside the sandbox and its localhost probe tests were
denied with `Operation not permitted`; the same complete command was rerun
outside the sandbox and passed. No dependency file or repository runtime state
was changed to satisfy either environment requirement.
