# Issues 176, 187, 188, 195, and 196 design

## Scope

Implement only the W6 H foundation row in the six GUI/help files assigned by
the dispatch. The merged Issue 338 commits are present at the branch parent.
The Trial execution screen, other pages, wizard, smoke scripts, and corpus
fixtures remain successor or parent-integration scope.

## Design

- Add shared, total formatting functions in `gui/lib/format.ts` for repository
  run state, Trial gate, Trial session status, phase status, and phase stage.
  Known wire values map to stable Japanese labels; missing or future values use
  an honest generic fallback instead of exposing a raw enum. Wire values remain
  unchanged for API, styling, and tests.
- Use the shared run and Trial formatters in the overview and Trial session
  index. A repository status tooltip must use the same localized label as the
  visible badge, so `status_text` cannot leak through an accessible name.
- Treat the shell navigation labels as the terminology authority: `概要`,
  `トライアル`, `拡張`, `リポジトリ実行記録`, and `計測`. Align the owned
  overview, getting-started, and session-history copy with those labels, and
  replace `execution root` with `実行ルート` in owned UI text.
- Put `aria-current="true"` on the highlighted Trial history row while
  preserving the existing visual `highlight` class. The overview selection is
  already exposed by the shell's `aria-current="page"` navigation contract.
- Make the runtime summary an atomic polite live region so availability,
  running-session, recovery, and fetch-failure transitions are announced
  without changing link semantics.
- Extend `docs/user/gui-help-map.md` with canonical terminology and formatter
  tables. This freezes the shared contract for successor rows that own the
  remaining Trial and wizard surfaces.

## Verification approach

Run a focused executable formatter assertion, GUI TypeScript type checking,
the internal-link lint, and a production GUI build. Also inspect the scoped
diff for raw owned UI values and confirm that only the six assigned files plus
this issue's three required reports changed. The existing browser smoke scripts
are intentionally not edited in this row because the dispatch assigns their
final updates to parent integration.
