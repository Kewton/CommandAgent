# 診断レポート: output/results.json のスキーマ検証失敗

## エラー引用

`results.json missing required key 'reconciliation'`

## 位置

`pipeline/main.py` (行末部分、結果辞書の構築箇所)

## コード引用

```python
results = {
    "aggregation": { ... },
    "summary": {
        "excluded_rows": { "invalid_format": 0, "missing_value": 2 },
        "input_rows": 60,
        "total_sales": 122438.0,
        "used_rows": 58
    }
}
with open(results_file, 'w') as f:
    json.dump(results, f, indent=2)
```

## 原因分析

### 1. スキーマ要件と現状の乖離

`evidence/results-schema.json` が示すデータ契約スキーマは `output/results.json` に `reconciliation` キーを必須としている。しかし、現在の `output/results.json` は `aggregation` と `summary` のみを含んでおり、`reconciliation` キーが存在しない。

**現在の output/results.json の構造:**
```json
{
  "aggregation": { ... },
  "summary": {
    "excluded_rows": { "invalid_format": 0, "missing_value": 2 },
    "input_rows": 60,
    "total_sales": 122438.0,
    "used_rows": 58
  }
}
```

**スキーマが求める構造:**
```json
{
  "aggregation": { ... },
  "summary": { ... },
  "reconciliation": {
    "input_rows": 60,
    "used_rows": 58,
    "excluded": [
      {"reason": "missing_value", "rows": 2}
    ]
  }
}
```

### 2. データの具体観測

`data/sales.csv` の検証から:
- 合計60行（ヘッダー含む）
- 欠損値: 2行（例: `,大阪,3000` — 日付フィールドが空）
- 無効な日付: `2026-02-30`（2月30日は存在しない日付）
- 有効な使用行数: 58行
- 除外行数: 2行（missing_value）

`output/report.md` の出力:
```
# Sales Summary Report
- Total Input Rows: 60
- Valid Rows Used: 58
- Excluded Rows:
  - invalid_format: 0
  - missing_value: 2
- Total Sales: 122438.00
```

### 3. 根本原因

`pipeline/main.py` の結果辞書構築ロジックは `aggregation` と `summary` のみを含める。`reconciliation` キー（`input_rows`, `used_rows`, `excluded` の配列形式）の生成処理が実装されていない。

`summary` 内に `input_rows` と `used_rows` は存在するが、スキーマは `reconciliation` キー配下に `excluded` を配列形式（`{"reason": "...", "rows": N}` のリスト）で要求している。

### 4. 追加の懸念: values キーの欠如

データ契約の `data_results_schema` 要件によれば、`report.md` に記載されるすべての数値クレームは `results.json` の `values` キー配下に明示的にエクスポートされる必要がある。現在の `output/results.json` には `values` キーが存在しない。

## 修正案・例示コードはコードブロックにせず

修正方針: pipeline/main.py の結果辞書構築箇所に `reconciliation` キーを追加する。`reconciliation` は `input_rows`, `used_rows`, `excluded`（`reason` と `rows` の辞書のリスト）を含む構造とする。また、`values` キーを追加し、各地域・月ごとの集計値をキー名（例: `regional_名古屋_2026-03`）でエクスポートする。

具体的には:
1. `reconciliation` キーを `results` 辞書に追加
2. `excluded` リストに `{"reason": "missing_value", "rows": 2}` を含める
3. `values` キーを追加し、`aggregation` の各値を `regional_{地域}_{月}` の形式でエクスポート
