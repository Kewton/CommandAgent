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
- Updated the focused document-drift and browser-smoke contracts for localized
  getting-started copy, overview badge/tooltips, Trial gate/status output,
  selected-session `aria-current`, and the shell live region. The session-index
  smoke now exercises runtime-badge reconnect from a separate shell page and
  records the resulting GET instead of clicking the already-loaded session.

## Scope control

Production and documentation edits are limited to the six paths assigned by
the dispatch. Test edits are limited to `tests/doc_drift.rs`,
`gui/scripts/session-index-smoke.mjs`, and directly corresponding assertions
in `gui/scripts/smoke.mjs`, as authorized by the revised dispatch. Trial
execution components, page/wizard scopes, corpus fixtures, and unrelated Rust
guards were not edited.

## Integration note

The formatter contract, GUI lint/type/build checks, focused document-drift
tests, both two-base-path browser smokes, Rust formatting, and Clippy pass. The
complete serial Rust run also passes all 2,124 library tests and the updated
document-drift target, then stops at three stale assertions in unowned
`tests/gui_read_only_guard.rs`. Those assertions require the old English Trial
availability label, the superseded overview formatter call shape, and a raw
session gate fallback. Verification remains blocked because satisfying those
assertions in owned production files would regress the approved contract, and
editing that guard is outside this row's authorized paths.
