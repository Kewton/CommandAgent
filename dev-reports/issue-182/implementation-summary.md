# Issue #182 Implementation Summary

## Outcome

The overview now explains its two run counts independently, renders concise
Japanese status badges, keeps unknown indexed states below the approved 20
percent ceiling, and exposes the recent-run ledger as a complete accessible
table. The production browser smoke records zero axe
`aria-required-children` violations at both supported base paths.

## Implementation

- Replaced the ambiguous `表示件数 / 総数` and `8 / 267` presentation with
  separate `最近の実行記録（一覧に表示中）` and
  `保存済みの実行記録（総数）` cards, each with a `件` unit.
- Added a typed presentation mapping from `RunState` to Japanese badges:
  `成功`, `失敗`, `進行中`, `記録あり`, `未記録`, and `判定不能`.
  The raw extracted status remains available as the badge title and through
  the existing API fields.
- Classified a preferred report without an explicit verdict as neutral
  `pending`/`recorded`, while a run with no preferred report remains
  `unknown`/`not recorded`. This does not infer or award pass/fail status.
- Rebuilt the ledger with `table` → `rowgroup` → `row` →
  `columnheader`/`cell` roles. Run-detail links remain native links inside
  cells instead of replacing their semantics with a row role.
- Pinned `axe-core` 4.10.3 as a test-only dependency and extended the existing
  two-base-path overview smoke to run the actual `aria-required-children`
  rule, assert Japanese badges, and enforce the 20 percent unknown ceiling.
- Updated the session-index smoke selector for semantic row containers and
  expanded Rust focused/static guards for the new contracts.

## Compatibility and exclusions

The six-field `RunSummary` JSON schema, raw `status` and `status_text` values,
event names and schemas, historical run evidence, and `.anvil/` runtime state
are unchanged. No corpus fixture changed because this work changes neither an
event/recovery contract nor a corpus contract.

The required Issue #148 predecessor commit was inspected before editing. Its
changes are CLI-only and introduce no relevant diff in the owned GUI/API/test
surface, so this branch did not copy unrelated predecessor commits.

## Observed acceptance evidence

The final overview smoke indexed 100 repository runs, classified 17 as
unknown (17 percent), displayed Japanese badges for every visible row, and
reported zero axe `aria-required-children` violations for both `/` and
`/proxy/commandagent/`. Visual inspection of the generated dashboard image
confirmed that the two cards and ledger layout remain intact.
