# GUI Trial history

[GUI index](gui.md) | [Trial guide](gui-trial.md) |
[Headless summaries](headless.md)

This page explains the compact `try/history/` execution-root session index and
how to compare pack runs without confusing Trial evidence with repository
management records. Terminal evidence belongs to the separate
`try/history/detail/?session=<id>` page.

## Two histories, two sources

**リポジトリ実行記録** projects repository-side
`workspace/management/runs`. **GUI Trial 実行履歴** projects
`.commandagent/runs` under the configured execution root, with `.anvil/runs`
retained as a legacy read source. Both screens show their source path. The
repository page does not discover Trial sessions, and the Trial index does not
rewrite historical management evidence.

The central run record and the mutable CLI workspace are intentionally
separate. Session `<session-id>` keeps confirmations, `events.jsonl`,
`summary.md`, and directive state in
`<execution-root>/.commandagent/runs/<session-id>/`, while generated code,
plans, evidence, repairs, and completion contracts live in
`<execution-root>/sessions/<session-id>/`. A later Gate 1 does not inventory
earlier session workspaces, and a continuation returns to the same session
workspace rather than the execution-root top level.

## Session rows and refresh

Entering or restoring a complete token loads up to 100 confirmed Trial run
directories. The active lease session appears first; other rows are ordered by
latest update. Each row is deliberately limited to execution/start and update
times, session ID, gate/status, profile, intent, and the persisted Gate 1 pack
pin or **選択なし**. It never infers identity from later mutable events.

The list loads fresh when the history page opens and revalidates on a runtime
lease transition, window focus/visibility, and **セッションを更新**. It has
no independent short-interval list poll. The page reports the latest successful
refresh time and whether its browser observation is fresh or stale.

A failed refresh leaves the last successful rows visible and reports the new
error separately. A missing/incomplete token is authentication pending, not an
authenticated empty history. A real empty history says
**確認済み GUI Trial セッションはありません。**

An in-flight row links to its read-only **実行状況** page. A terminal row links
to **結果詳細**, which owns the terminal verdict, failure diagnosis,
acceptance, events, and artifact links. Failure diagnosis is never expanded
inside the history row. Returning from detail uses the session fragment only
to focus its compact history row.

## Lease projection

The same index response projects `idle`, `running(<session-id>)`, or
`recovery_required(<session-id>)`. A non-idle snapshot disables confirmed
launch and names the owning/blocking session. This client snapshot is advisory
and read-only; the server enforces the lease on POST. The GUI exposes no clear,
reset, cancel, or force-idle action.

Use the conservative [workspace recovery procedure](gui-trial.md#workspace-lease-inspection-and-recovery)
when recovery is required.

## Pack columns and A/B comparison

The pack column is the exact Gate 1 identity: `id@version`, source label, and
the confirmation's persisted hash. For A/B:

1. Hold goal, workspace fixture, profile, intent, providers/models, and suite
   constant.
2. Confirm run A with one exact pack version and run B with the other.
3. Open each row's result detail and compare its Gate 1 identity, `summary.md`,
   event evidence, verdict, assurance, duration, and the same acceptance checks.
4. Treat the comparison as observational unless the measurement protocol fixes
   repetition, population, and uncertainty. A single GUI pair is not a band.

The history screen does not automatically score two rows or claim causality.
For scripted callers, the [headless summary](headless.md) exposes the same
persisted pack identity and terminal facts.

## Reference source

Run lifecycle, pack pins, and acceptance come from CLI-owned files below the
execution root. The GUI server only reads them. Delegated stdout/stderr are not
saved by this path; use `events.jsonl` and `summary.md`. If an unstructured log
is ever required, it must become a CLI-owned output contract rather than a GUI
server write.
