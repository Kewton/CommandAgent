# Profile Contract Binding: ingest (admitted)

正準契約は [`docs/ingest-profile-contract.md`](../../../../../docs/ingest-profile-contract.md)
（fixed v0.1 2026-07-28、v0 fixed 2026-07-25）。conformance 6負例＋full正例、
全履歴の正直終端と偽成功ゼロ、elev-008の機械クラス0/6を根拠に
`admission = "admitted"`へ昇格した。

## 1. スコープ

保存済みHTML/text snapshotから宣言レコードを抽出・整形する第1段のみ。
network取得・鮮度は第2段QUEUED。

## 2. full の意味（最重要・不変条件）

N1実行、N2全field source binding、N3候補勘定、N4宣言format、
N5再実行一致が全成立すること。

## 3. 要求 evidence（full の必須ゲート）

N1 `pipeline_probe`、N2 `source_binding`（同一候補・値保存形式変換・
三条件つき文書共有文脈補完）、N3 `accounting`、
N4 `format_schema`、N5 `rerun`。詳細schemaは正準契約に従う。

## 4. assurance 階層

full=N1〜N5 pass、partial=N1 passかつN2 claims_absent、
static=N1未実行、failed=N2捏造/改変・N3勘定・N4schema・N5再実行の違反。
admitted後はearned full/partialをそのまま表示し、failedはfailedを維持する。

## 5. 実行プローブ

N3 freeze後、N1を隔離・有界・offlineで実行し、N2〜N4を観測、
N5で同一entryを再実行する。

## 6. 偽装耐性（conformance ネガティブテストの要求）

捏造record、値改変、silent drop、schema外、未実行、候補集合縮小の
6形を `tests/ingest_profile_conformance.rs` で拒否する。

## 7. スコープ外（明示）

source全eventの完全発見、動的rendering、取得・鮮度、network fetch。

## 8. 生成側への制約

snapshot固定、network禁止、候補selectorとrecord formatの事前宣言、
値保存・決定的宣言・field別記録の三条件を満たす正規化のみ許可する。
文書共有文脈補完では候補内断片と候補外文書文脈断片の両位置を記録し、
候補間の値の継ぎ合わせは許可しない。
