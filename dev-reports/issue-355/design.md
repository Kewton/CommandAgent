# Issue #355 design

## Problem

GUI Trial の静的ビルドと `gui_server` が異なる API 契約を使うと、旧応答に
`identity` がないだけで terminal の描画が例外になり、FAILED の原因を読む前に
Next.js の既定エラー画面へ落ちる。また、履歴一覧は FAILED という状態だけを返し、
詳細 API も `stop_reason` 以外の release gate / probe 診断を投影しない。
さらに一覧と証跡 API は legacy の `.anvil/runs` を読める一方、詳細 API は
canonical の `.commandagent/runs` に固定されている。

## Predecessor baseline

Issue #353 (`6b81cde5`) と Issue #354 worktree の committed chain
(`b154ea19`, `b1ff8e40`, `48fd6d30`) を確認した。#354 は #352 のセッション別
execution workspace と #353 の provider role 分離を親に持つため、この chain を先に
取り込み、その tree を #355 の実装基準にする。

## Design

1. GUI/server 契約版を 1 つの committed JSON manifest に固定する。Next.js はその値を
   bundle に取り込み、`gui_server` は同じ manifest を compile time に読み込む。
   runtime-status は server 契約版を返し、Shell は欠落または不一致を日本語バナーで
   表示する。`--check` は static export 内の manifest の存在・構文・一致を独立 check
   として検証する。
2. `PolledSession.identity` はクライアント上で optional として扱い、旧 server 応答では
   terminal を維持したまま再ビルド/再起動を案内する。これは smoke で旧応答を固定する。
3. session event projection に additive な `failure_diagnostics` を追加する。
   `stop_reason`、`release_gate_reasons`、probe status/reasons/evidence path を bounded に
   抽出し、履歴行と terminal の両方に表示する。既存 event 名や schema は変更しない。
4. 読み取り用 session path resolution は canonical `.commandagent/runs` を優先し、なければ
   legacy `.anvil/runs` を選ぶ。詳細・一覧・証跡の read-only containment/symlink guards は
   維持し、新規 run の write path は canonical のままにする。

## Tests and verification

- Rust integration testsで static manifest の一致/不一致、legacy `.anvil/runs` の詳細・
  証跡、FAILED diagnostics projection を固定する。
- GUI smoke で `identity` 欠落時の日本語案内、契約版不一致バナー、一覧/terminal の
  FAILED 原因表示を固定する。
- focused checks の後、Issue 指定の Rust/GUI checks をすべて実行する。
