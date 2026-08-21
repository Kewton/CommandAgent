# Issue #182 Design

## Scope and predecessor

- Apply the authoritative Epic #260 Lane H combined decision for Issues #182,
  #183, and #193 on this branch: clarify overview metrics, reduce unclassified
  overview states to at most 20 percent with Japanese badges, and restore the
  run ledger's accessible table semantics.
- Limit production changes to `gui/app/page.tsx` and status classification in
  `src/bin/gui_server/api.rs`. Preserve the `RunSummary` JSON schema, raw
  `status`/`status_text` values, event contracts, historical run evidence, and
  the live `.anvil/` namespace.
- The required Issue #148 predecessor tip (`16d5a854`) was inspected before
  editing. It changes only CLI Gate 4 presentation and reports; its relevant
  GUI/API/test files are byte-identical to this branch, so no predecessor
  commit needs to be incorporated into this scoped change.

## Design

1. Replace the combined `8 / 267` value with two numeric cards whose labels
   independently identify the recently displayed count and the repository's
   saved total. Keep the existing `run-count` hook on the displayed count and
   add a separate total-count hook for focused browser verification.
2. Keep raw extracted status text in the API for compatibility, but present a
   short Japanese badge derived from the typed `RunState`: `成功`, `失敗`,
   `進行中`, `記録あり`, `未記録`, or `判定不能`.
3. Treat a preferred report with no explicit `Task status:`, `Status:`, or
   `Overall:` field as a neutral recorded/pending state. Continue to classify a
   run with no preferred report as unknown. This makes the state reflect the
   evidence actually available without inferring success or failure and keeps
   the current repository's unknown share at or below 20 percent.
4. Use a complete ARIA table hierarchy: `table` contains header/body
   `rowgroup`s, each contains `row`s, and every row contains the required
   `columnheader` or `cell` children. Keep run-detail navigation as ordinary
   links inside cells so link semantics are not overridden by table roles.

## Tests and verification

- Update focused Rust unit/integration coverage for explicit statuses, recorded
  reports, missing reports, schema compatibility, and the 20 percent ceiling.
- Update the static GUI guard and overview browser smoke assertions for the two
  metrics, Japanese badges, and complete table-role hierarchy.
- Run the focused API tests and GUI checks first. Because shared GUI/API code is
  touched, then run formatting, Clippy, and the full Rust suite.
