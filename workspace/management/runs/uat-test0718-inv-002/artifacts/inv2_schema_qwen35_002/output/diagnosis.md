# 診断レポート: output/results.json のスキーマ検証失敗

## エラー引用

`ValueError: results.json missing required key 'reconciliation'`

## 位置

`output/results.json` のトップレベルキー構成

## 原因分析

### 1. 実際の出力構造（`output/results.json`）

```json
{
  "aggregation": {
    "名古屋": {"2026-03": 20730.0, "2026-06": 19767.0},
    "大阪": {"2026-02": 18657.0, "2026-05": 21470.0},
    "東京": {"2026-01": 19990.0, "2026-02": 5000.0, "2026-04": 16824.0}
  },
  "summary": {
    "excluded_rows": {"invalid_format": 0, "missing_value": 2},
    "input_rows": 60,
    "total_sales": 122438.0,
    "used_rows": 58
  }
}
```

### 2. 要求されるデータ契約スキーマ

```json
{
  "reconciliation": {
    "input_rows": N,
    "used_rows": N,
    "excluded": [{"reason": "...", "rows": N}]
  },
  "values": {
    "<claim_key>": number
  }
}
```

### 3. 不整合の特定

`pipeline/main.py` は `output/results.json` を出力する際、`aggregation` と `summary` の2つのキーのみを使用している。しかし、データ契約（`evidence/results-schema.json`）は `reconciliation` キーの存在を必須としている。

具体的には以下の2点でスキーマ違反が発生している:

- **`reconciliation` キーの欠落**: `summary` に `input_rows: 60` と `used_rows: 58` が存在するが、これらは `reconciliation` キーの下に配置されるべき。また `excluded_rows` は `excluded` キー配列（`{"reason": "...", "rows": N}` 形式）として再構成される必要がある。
- **`values` キーの欠落**: `report.md` に記載されているすべての数値主張（`total_sales: 122438.0`、各地域・月の売上など）は `values` キー配下に数値としてエクスポートされるべきだが、現状では `aggregation` キーにのみ存在する。

### 4. 再現手順

1. `pipeline/main.py` を実行する
2. 出力ファイル `output/results.json` が生成される
3. `output/results.json` のトップレベルキーを `["aggregation", "summary"]` とする
4. スキーマバリデーターが `reconciliation` キーの欠如を検出し、検証失敗を返す

### 5. 実データに基づく観測値

- `data/sales.csv` には60行のデータ（ヘッダーを含む）
- 除外行数: `missing_value: 2`（`2026-02-30` という不正な日付と `region` が空の行）
- 使用行数: 58行
- 合計売上: 122438.0
- 地域別売上: 名古屋（2026-03: 20730.0, 2026-06: 19767.0）、大阪（2026-02: 18657.0, 2026-05: 21470.0）、東京（2026-01: 19990.0, 2026-02: 5000.0, 2026-04: 16824.0）

### 6. 修正方針

`pipeline/main.py` の出力ロジックを変更し、`aggregation` と `summary` の代わりに、`reconciliation` キー（`input_rows`、`used_rows`、`excluded` 配列）と `values` キー（すべての数値主張をキー・バリューペアとして）を含む構造を出力する必要がある。具体的には、`summary` の `input_rows` と `used_rows` を `reconciliation` に移動し、`excluded_rows` を `excluded` 配列形式に変換し、`aggregation` の各値と `total_sales` を `values` キーにエクスポートする。
