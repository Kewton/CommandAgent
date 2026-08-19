# GUI help-to-document map

This short table maps stable in-app Japanese explanations, term help, empty
states, and actions to one owning section. `gui/scripts/smoke.mjs` checks the
copy on both supported base paths.

| Kind | In-app copy | Source | Document owner |
| --- | --- | --- | --- |
| explanation | 前提を確認し、サンプル目標から Gate 1 の実行前確認を試せます。 | `gui/components/getting-started.tsx` | [`getting-started-gui.md#はじめに`](getting-started-gui.md#はじめに) |
| term help | CLI を動かす前に、目標・変更範囲・検証条件を確認する段階です。 | `gui/components/getting-started.tsx` | [`getting-started-gui.md#terms-shown-in-the-app`](getting-started-gui.md#terms-shown-in-the-app) |
| term help | Trial がファイルを変更できる、専用の作業ディレクトリです。 | `gui/components/getting-started.tsx` | [`getting-started-gui.md#terms-shown-in-the-app`](getting-started-gui.md#terms-shown-in-the-app) |
| term help | 目標に追加する検証知識。選択した版とハッシュが確認内容に固定されます。 | `gui/components/getting-started.tsx` | [`gui-trial.md#pack-selection-and-frozen-identity`](gui-trial.md#pack-selection-and-frozen-identity) |
| Gate primer | Gate 1 は CLI 実行前の確認です | `gui/components/trial-run.tsx` | [`gui-trial.md#gate-1-confirm-before-execution`](gui-trial.md#gate-1-confirm-before-execution) |
| empty state | 固定済みパックが見つかりません。 | `gui/app/assets/page.tsx` | [`gui-extensions.md#extensions-catalog`](gui-extensions.md#extensions-catalog) |
| action | Trial で使う | `gui/app/assets/page.tsx` | [`gui-extensions.md#extensions-catalog`](gui-extensions.md#extensions-catalog) |
| empty state | 確認済み GUI Trial セッションはありません。 | `gui/components/trial-session-index.tsx` | [`gui-history.md#session-rows-and-refresh`](gui-history.md#session-rows-and-refresh) |
