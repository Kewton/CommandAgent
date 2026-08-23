# Issues 176, 187, 188, 195, and 196 implementation summary

## Implemented

- Added shared Japanese display formatters for repository run state, Trial
  gates, Trial session status, phase status, and phase stage. Unknown wire
  values now use explicit fallback labels instead of leaking raw enums.
- Updated the overview to use the shared repository status formatter for both
  visible badges and accessible tooltip text. The owned overview copy now uses
  the navigation terms `概要`, `拡張`, and `リポジトリ実行記録` consistently.
- Localized the getting-started card's English headings, Trial references,
  `execution root`, and `pack` terminology.
- Localized the Trial session index heading, explanatory/error/empty copy,
  pack label, and raw gate/status pair. The highlighted history row now exposes
  `aria-current="true"` while retaining its visual class.
- Made the shell runtime summary an atomic polite live region. Its Trial
  availability labels are Japanese, and the existing overview navigation
  selection remains exposed through `aria-current="page"`.
- Added canonical terminology and status-label tables to
  `docs/user/gui-help-map.md` so successor rows have one shared display
  contract for their remaining Trial and wizard surfaces.

## Scope control

Production and documentation edits are limited to the six paths assigned by
the dispatch. The only additional files are the three required reports under
`dev-reports/issue-176/`. Trial execution components, page/wizard scopes,
corpus fixtures, smoke scripts, and Rust test harnesses were not edited.

## Integration note

The owned implementation, formatter assertions, GUI type/build checks, Rust
formatting, Clippy, and the serial GUI-server integration target pass. The
broader serial Rust suite reaches the parent-owned `tests/doc_drift.rs` harness
and fails because that harness still requires the replaced English getting-
started sentence. Existing browser smoke assertions likewise still expect raw
session labels such as `GATE_2 / RUNNING`. Updating those final harness
contracts belongs to parent integration under the approved row decision, so
verification is recorded as blocked rather than weakening the new UI contract
or editing outside the owned paths.
