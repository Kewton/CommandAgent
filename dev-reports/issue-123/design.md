# Issue 123 design

## Goal

E-3/E-4およびE-17/E-18の実装者ではない担当として、E-18の外部draft
profileを1セル追加し、拡張作業そのものと記帳作業を分けて実測する。

## Target and protocol

- 対象セルは`landing-page`（`create` intent、`Quiz` family）とする。
- 2026-08-20T02:43:16+0900を実測開始時刻とする。
- Issue指定どおり`workspace/management/scripts/scaffold.py profile
  landing-page`から開始し、生成された`ADMISSION.md`とE-18の外部manifest契約を
  照合する。
- 実行可能なセル本体は新規証跡
  `workspace/management/runs/20260820-bp1-one-cell/extension-root/`に置き、既存run、
  `.anvil/`、production code、event schemaは変更しない。
- loader/doctorと既存E-18 focused testでmanifestのstrict load、draft強制、既存
  runtime契約を確認する。共通完了条件のfmt、Clippy、全testも実行する。

## Measurements

`git diff --stat`とpath一覧から、(1)セル追加、(2)scaffold由来、(3)Issue記帳を
別集計する。工数は開始から最終検証までのagent wall-clockとして記録し、人間の
person-hoursとは主張しない。計測コストはprovider/API呼び出し、外部課金、実行した
検証コマンドと所要時間を記録する。

## Scope recommendation basis

既存catalog capabilityだけでセルを成立させられたか、scaffoldがE-18形式へ直接到達
できたか、overlayが必要だったかを観測し、E-17のcatalog拡張条件とE-18のoverlay
許可範囲に対する提案を親Issue #103へコメントする。
