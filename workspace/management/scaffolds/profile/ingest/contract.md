# Profile Contract Binding: ingest (fixed — admission off)

正準契約は [`docs/ingest-profile-contract.md`](../../../../../docs/ingest-profile-contract.md)
（fixed 2026-07-25）。実装済みだが、実run corpusとreviewer admissionが
未完了のため `admission = "off"` を維持する。

## 1. スコープ

保存済みHTML/text snapshotから宣言レコードを抽出・整形する第1段のみ。
network取得・鮮度は第2段QUEUED。

## 2. full の意味（最重要・不変条件）

N1実行、N2全field source binding、N3候補勘定、N4宣言format、
N5再実行一致が全成立すること。

## 3. 要求 evidence（full の必須ゲート）

N1 `pipeline_probe`、N2 `source_binding`、N3 `accounting`、
N4 `format_schema`、N5 `rerun`。詳細schemaは正準契約に従う。

## 4. assurance 階層

full=N1〜N5 pass、partial=N1 passかつN2 claims_absent、
static=N1未実行、failed=N2捏造/改変・N3勘定・N4schema・N5再実行の違反。
admission off中はearned full/partialを表示上staticへcapする。

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
