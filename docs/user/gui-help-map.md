# GUI help-to-document map

This short table maps stable in-app Japanese explanations, term help, empty
states, and actions to one owning section. `gui/scripts/smoke.mjs` checks the
copy on both supported base paths.

## Canonical terminology

Navigation labels are the authority for screen and feature names. Successor
GUI work uses these terms instead of introducing another label for the same
concept.

| Concept | Canonical in-app term |
| --- | --- |
| repository evidence overview | 概要 |
| delegated GUI run | トライアル |
| Trial request and Gate 1 | 実行指示 |
| read-only in-flight Trial | 実行状況 |
| compact Trial session list | 実行履歴 |
| terminal Trial evidence | 結果詳細 |
| four-layer extension boundary and catalogs | 拡張 |
| contracts and measurement suites | 参照資料（拡張内） |
| `workspace/management/runs` history | リポジトリ実行記録 |
| reports and bands | 計測 |
| `.commandagent/runs` session history | トライアル実行履歴 |
| configured Trial workspace | 実行ルート |
| pinned verification knowledge | パック |

## Shared status labels

`gui/lib/format.ts` owns these display mappings. API values, data attributes,
and CSS hooks keep the raw wire value; visible and accessible copy uses the
label or the documented unknown fallback.

| Kind | Wire value | In-app label |
| --- | --- | --- |
| Trial gate | `gate_1` | Gate 1（実行前確認） |
| Trial gate | `gate_2` | Gate 2（実行） |
| Trial gate | `gate_3` | Gate 3（完了） |
| Trial gate | `gate_4` | Gate 4（要対応） |
| Trial gate | missing / future value | Gate 未確定 / Gate 不明 |
| Trial status | `starting` | 開始中 |
| Trial status | `running` | 実行中 |
| Trial status | `completed` | 完了 |
| Trial status | `failed` | 失敗 |
| Trial status | `interrupted` | 中断 |
| Trial status | `aborted` | 中止 |
| Trial status | `pending` | 待機中 |
| Trial status | `incomplete` | 未完了 |
| Trial status | `unreadable` | 読み取り不可 |
| Trial status | missing / future value | 状態不明 |
| phase status | `pending` / `running` / `completed` | 待機中 / 実行中 / 完了 |
| phase status | `failed` / `interrupted` / `aborted` | 失敗 / 中断 / 中止 |
| phase status | missing / future value | 状態不明 |
| phase stage | queued / start / scaffold / lint / execute | 待機中 / 開始準備中 / 計画中 / 計画を確認中 / 実装中 |
| phase stage | verification / profile / complete | 検証中 / プロファイルを検証中 / 完了 |
| phase stage | missing / future value | 段階不明 |

## Help ownership

Rows owned by successor GUI work retain their currently implemented copy here
until that source is migrated. The canonical terminology table above defines
the target vocabulary for those migrations.

| Kind | In-app copy | Source | Document owner |
| --- | --- | --- | --- |
| explanation | 前提を確認し、サンプル目標から Gate 1 の実行前確認を試せます。 | `gui/components/getting-started.tsx` | [`getting-started-gui.md#はじめに`](getting-started-gui.md#はじめに) |
| heading | 初回案内 / はじめに | `gui/components/getting-started.tsx` | [`getting-started-gui.md#はじめに`](getting-started-gui.md#はじめに) |
| action | サンプル目標をトライアルに入力 | `gui/components/getting-started.tsx` | [`getting-started-gui.md#first-trial-walkthrough`](getting-started-gui.md#first-trial-walkthrough) |
| term help | CLI を動かす前に、目標・変更範囲・検証条件を確認する段階です。 | `gui/components/getting-started.tsx` | [`getting-started-gui.md#terms-shown-in-the-app`](getting-started-gui.md#terms-shown-in-the-app) |
| term help | トライアルがファイルを変更できる、専用の作業ディレクトリです。 | `gui/components/getting-started.tsx` | [`getting-started-gui.md#terms-shown-in-the-app`](getting-started-gui.md#terms-shown-in-the-app) |
| term help | 目標に追加する検証知識。選択した版とハッシュが確認内容に固定されます。 | `gui/components/getting-started.tsx` | [`gui-trial.md#pack-selection-and-frozen-identity`](gui-trial.md#pack-selection-and-frozen-identity) |
| Gate primer | Gate 1 は CLI 実行前の確認です | `gui/components/trial-compose.tsx` | [`gui-trial.md#gate-1-confirm-before-execution`](gui-trial.md#gate-1-confirm-before-execution) |
| result guidance | 実行結果と次の一手を確認してください | `gui/components/trial-terminal.tsx` | [`gui-trial.md#gate-34-read-the-result`](gui-trial.md#gate-34-read-the-result) |
| result guidance | 独立した CLI 動作プローブは実行されていません。 | `gui/components/trial-terminal.tsx` | [`gui-trial.md#gate-34-read-the-result`](gui-trial.md#gate-34-read-the-result) |
| action | 受入シートの詳細を表示 | `gui/components/trial-terminal.tsx` | [`gui-trial.md#gate-34-read-the-result`](gui-trial.md#gate-34-read-the-result) |
| empty state | 固定済みパックが見つかりません。 | `gui/app/assets/page.tsx` | [`gui-extensions.md#extensions-catalog`](gui-extensions.md#extensions-catalog) |
| action | トライアルで使う | `gui/app/assets/page.tsx` | [`gui-extensions.md#extensions-catalog`](gui-extensions.md#extensions-catalog) |
| heading | 4 レイヤーと依存関係 | `gui/app/assets/page.tsx` | [`gui-extensions.md#four-extension-layers`](gui-extensions.md#four-extension-layers) |
| action | 安全な登録 Issue を作る | `gui/app/assets/page.tsx` | [`gui-extensions.md#layer-2-draft-profiles`](gui-extensions.md#layer-2-draft-profiles) |
| heading | Contract / Suite は拡張種別ではありません | `gui/app/assets/page.tsx` | [`gui-extensions.md#contract-and-suite-references`](gui-extensions.md#contract-and-suite-references) |
| action | パック作成ウィザードを開く | `gui/components/pack-wizard.tsx` | [`gui-extensions.md#pack-creation-wizard`](gui-extensions.md#pack-creation-wizard) |
| heading | プロファイル登録ウィザード | `gui/components/profile-wizard.tsx` | [`gui-extensions.md#draft-profile-registration-wizard`](gui-extensions.md#draft-profile-registration-wizard) |
| empty state | 確認済みのトライアルセッションはありません。 | `gui/components/trial-session-index.tsx` | [`gui-history.md#session-rows-and-refresh`](gui-history.md#session-rows-and-refresh) |
