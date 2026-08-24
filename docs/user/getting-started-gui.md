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

## Overview landing page

The **概要** page starts with **目標を、検証可能なコードに。** and the
primary **トライアルを始める** action. It explains the product before
showing operational state: CommandAgent confirms the change boundary before
execution, delegates the CLI inside a dedicated execution root, verifies the
result, and preserves evidence for both success and failure. When a running or
recovery-required session is reported by `runtime-status`, a second action
opens that exact session under `/try/status/?session=<id>`.

Detailed capability maps and bands live under **計測**. The
`workspace/management/runs` list lives under **リポジトリ実行記録**. Overview
keeps links to both pages but does not duplicate their maps, counts, band rows,
or run table.

## Safety and honest results

The four principles on Overview are behavior, not marketing health indicators:

- **local-first** keeps code and evidence in the trusted local environment;
- **Gate 1** is the explicit pre-execution confirmation of the goal, write
  boundary, model identities, and required checks;
- the **execution root** bounds Trial writes and stays disjoint from repository
  and extension roots; and
- verification, bounded repair, and evidence support **honest failure**. A
  missing or failed required check is never presented as verified success.

In the plain-language terms, a **profile** selects task guidance and the
minimum check floor, a **pack** adds exact-version/hash-pinned knowledge, and
**assurance** is earned from checks and evidence rather than configuration.

## Goal to verified result

Overview presents the same five-step contract used by the runtime:

1. **Goal** — enter the desired creation, fix, or investigation in ordinary
   language.
2. **実行前確認** — review and confirm Gate 1 before the CLI starts.
3. **計画と実装** — plan and change code only inside the confirmed boundary.
4. **検証と修復** — run the required checks and return failures to bounded
   repair without discarding evidence.
5. **Result** — report a verified result only when every required condition is
   met; otherwise report failure or required action with its evidence.

## はじめに

The persistent **FIRST USE / はじめに** section reports **Trial の作業場所**,
**非公開の拡張ルート**, **CommandAgent CLI**, and **Trial アクセス** from
`runtime-status` as **準備済み**, **未設定**, or **要対応**. It does not
cache or dismiss those facts. Resolve every actionable Trial item before
delegating a run; an extension-root issue blocks local extensions but does not
turn another prerequisite green.

The four route cards match the fixed Trial ownership boundaries:

- `/try/` — **実行指示** and Gate 1;
- `/try/status/?session=<id>` — read-only **実行状況**;
- `/try/history/` — compact **実行履歴**; and
- `/try/history/detail/?session=<id>` — terminal **結果詳細** and evidence.

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
   hash and enforces the single-workspace lease. The browser then moves from
   **実行指示** to the session's read-only **実行状況** page.
8. During **実行状況**, read execution state separately from monitoring health.
   A lost browser connection does not imply that the CLI stopped. Reloading the
   route reconnects; another tab can reconnect after entering its own token.
9. Terminal state moves to **結果詳細**. Inspect verdict, assurance, status,
   failure diagnosis, acceptance, recent events, and artifacts as separate
   facts. Gate 4 means preserve the evidence and choose a recovery action; it
   does not authorize weaker verification.
10. Use **実行履歴** for the compact session list. It shows summary identity and
    state only; open a row for live status or terminal detail.

The complete behavior, reconnect boundary, and Gate 4 controls are in the
[Trial guide](gui-trial.md). Existing sessions and pack pins are explained in
[Trial history](gui-history.md).

## Live readiness and session state

The **現在の状態** section renders only `runtime-status` data: overall Trial
readiness, the active/recovery session, and the redacted extension-root state.
Loading, unavailable, unconfigured, and action-required states have distinct
labels. A failed request is shown as **状態取得失敗** and is never converted to
an always-green badge. The extension-root absolute path is not exposed.

## Terms shown in the app

| App term | Meaning |
| --- | --- |
| Gate 1 | Confirm goal, change boundary, exact identity, and checks before CLI execution. |
| execution root | The dedicated directory the delegated Trial may change. It must be disjoint from repository and extension roots. |
| profile | Task guidance plus the minimum set of checks required for that task family. |
| pack | Additional verification knowledge pinned by exact version and hash in the confirmation. |
| assurance | The guarantee level earned from checks and evidence; configuration alone cannot raise it. |

The [GUI help map](gui-help-map.md) binds these explanations and empty-state
messages to their document sections and browser-smoke assertions.
