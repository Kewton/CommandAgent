# Getting started with the GUI

[GUI index](gui.md) | [Setup](gui-setup.md) | [Trial guide](gui-trial.md)

The CommandAgent GUI projects repository evidence into read-only views and can
delegate one confirmed CLI Trial in a dedicated execution root. It never calls
a provider or runner in the GUI process. This page follows the Japanese labels
shown by the application.

## Prerequisites

- Rust 1.88 or later
- Node.js 20.9 or later
- npm using the committed `gui/package-lock.json`
- an existing, trusted Trial execution root that is separate from this repository
- the exact `commandagent` product binary and provider model IDs you intend to use

Install the pinned GUI dependency graph with `npm ci --include=dev` from
`gui/`. For a guided build and preflight, use the
[GUI setup workflow](gui-setup.md#guided-setup-and-preflight).

## はじめに

When the overview first opens, **はじめに** appears above the dashboard. Its
`runtime-status` check reports the **Trial の作業場所**, **commandagent CLI**,
and **Trial アクセス** as **準備済み**, **未設定**, or **要対応**. Resolve
every actionable item before delegating a run.

**閉じる** hides the card for the current browser tab, including reload and
navigation. Another tab has an independent state. Closing the card changes no
server setting and grants no authority.

## First Trial walkthrough

1. Select **サンプル目標を Trial に入力**. The GUI fills a small Python CLI
   goal, the `python-cli` profile, and an admitted pack. It deliberately leaves
   the executor and planner model IDs empty.
2. If token authentication is on, enter the runtime-only **Trial access token**.
   It is tab-scoped; it is never placed in the URL or static export.
3. In each **Executor / 実行** and **Planner / 計画** group, select the provider
   and enter its exact model ID. Each pair shares one row on desktop; on mobile,
   the model immediately follows its provider. Changing either provider does
   not rewrite the corresponding model pin.
4. Leave **実行目的** at **自動判定** for request-word compatibility, or
   explicitly select **作成**, **修正**, or **調査**. An explicit choice is
   frozen at Gate 1 even when the goal contains words associated with another
   intent.
5. Select **契約と見積りを確認**. This asks the server for a Gate 1 card; it
   does not launch the CLI.
6. Read the goal, profile, intent, models, pack version/hash/source, write
   boundary, and every required check. The app reminder is explicit:
   **Gate 1 は CLI 実行前の確認です**.
7. Select the confirmation checkbox only when the card is correct, then choose
   **確認して CLI を実行**. The server independently requires the exact card
   hash and enforces the single-workspace lease.
8. During **実行**, read execution state separately from monitoring health.
   A lost browser connection does not imply that the CLI stopped.
9. Under **結果**, inspect verdict, assurance, status, recent events, and
   artifacts as separate facts. Gate 4 means preserve the evidence and choose
   a recovery action; it does not authorize weaker verification.

The complete behavior, reconnect boundary, and Gate 4 controls are in the
[Trial guide](gui-trial.md). Existing sessions and pack pins are explained in
[Trial history](gui-history.md).

## Terms shown in the app

| App term | Meaning |
| --- | --- |
| Gate 1 | Confirm goal, change boundary, exact identity, and checks before CLI execution. |
| execution root | The dedicated directory the delegated Trial may change. It must be disjoint from repository and extension roots. |
| pack | Additional verification knowledge pinned by exact version and hash in the confirmation. |

The [GUI help map](gui-help-map.md) binds these explanations and empty-state
messages to their document sections and browser-smoke assertions.
