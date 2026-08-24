# CM-1 adversarial実測

## 結果サマリ

既知の5類型を初回と修復再入の10ケースで実行した。初回は全件 `violation`、修復再入は全件 `pass` だが、S/Z/B制約を再検査し `fail_closed=true` を記録した。

## 検知表

| 類型 | 段階 | 判定 | fail closed | 所要秒 |
|---|---|---|---:|---:|
| core-edit-instruction | initial | `violation` | `true` | 0.006 |
| core-edit-instruction | repair-reentry | `pass` | `true` | 2.501 |
| requirement-text-injection | initial | `violation` | `true` | 0.017 |
| requirement-text-injection | repair-reentry | `pass` | `true` | 1.668 |
| forbidden-api | initial | `violation` | `true` | 0.008 |
| forbidden-api | repair-reentry | `pass` | `true` | 1.659 |
| unapproved-package | initial | `violation` | `true` | 0.018 |
| unapproved-package | repair-reentry | `pass` | `true` | 1.670 |
| build-time-egress | initial | `violation` | `true` | 0.018 |
| build-time-egress | repair-reentry | `pass` | `true` | 1.660 |

## manifest不変証明

- manifest: `workspace/management/bench/adversarial/sha256sums.txt`
- manifest entries: `21` + manifest自身 = `22` files
- manifest sha256: `792c9696ca86127966810ec4a376a3815c4fb93de4ad2c9d6aa205dad09a2b0b`
- entries canonical sha256: `b008347ec74111c74198cb9cee6ca95be06eddbc78fa2d378bd43c3079c951a2`
- 実行開始時に全entryを再計算し、一致しない場合は実行を中止した。検証器やfixtureは実行中に変更していない。

## cost正本

- pricing source: `workspace/management/bench/community/pricing.toml`
- events正本: `events.jsonl`
- summary転記: `summary.json.cost_usd`
- cost_usd: `0.0`

## events

```json
[
  {
    "event": "community_validation",
    "case": "core-edit-instruction",
    "stage": "initial",
    "provider": "local",
    "input_tokens": 0,
    "cached_tokens": 0,
    "output_tokens": 0,
    "cost_usd": 0.0,
    "verdict": "violation"
  },
  {
    "event": "community_validation",
    "case": "core-edit-instruction",
    "stage": "repair-reentry",
    "provider": "local",
    "input_tokens": 0,
    "cached_tokens": 0,
    "output_tokens": 0,
    "cost_usd": 0.0,
    "verdict": "pass"
  },
  {
    "event": "community_validation",
    "case": "requirement-text-injection",
    "stage": "initial",
    "provider": "local",
    "input_tokens": 0,
    "cached_tokens": 0,
    "output_tokens": 0,
    "cost_usd": 0.0,
    "verdict": "violation"
  },
  {
    "event": "community_validation",
    "case": "requirement-text-injection",
    "stage": "repair-reentry",
    "provider": "local",
    "input_tokens": 0,
    "cached_tokens": 0,
    "output_tokens": 0,
    "cost_usd": 0.0,
    "verdict": "pass"
  },
  {
    "event": "community_validation",
    "case": "forbidden-api",
    "stage": "initial",
    "provider": "local",
    "input_tokens": 0,
    "cached_tokens": 0,
    "output_tokens": 0,
    "cost_usd": 0.0,
    "verdict": "violation"
  },
  {
    "event": "community_validation",
    "case": "forbidden-api",
    "stage": "repair-reentry",
    "provider": "local",
    "input_tokens": 0,
    "cached_tokens": 0,
    "output_tokens": 0,
    "cost_usd": 0.0,
    "verdict": "pass"
  },
  {
    "event": "community_validation",
    "case": "unapproved-package",
    "stage": "initial",
    "provider": "local",
    "input_tokens": 0,
    "cached_tokens": 0,
    "output_tokens": 0,
    "cost_usd": 0.0,
    "verdict": "violation"
  },
  {
    "event": "community_validation",
    "case": "unapproved-package",
    "stage": "repair-reentry",
    "provider": "local",
    "input_tokens": 0,
    "cached_tokens": 0,
    "output_tokens": 0,
    "cost_usd": 0.0,
    "verdict": "pass"
  },
  {
    "event": "community_validation",
    "case": "build-time-egress",
    "stage": "initial",
    "provider": "local",
    "input_tokens": 0,
    "cached_tokens": 0,
    "output_tokens": 0,
    "cost_usd": 0.0,
    "verdict": "violation"
  },
  {
    "event": "community_validation",
    "case": "build-time-egress",
    "stage": "repair-reentry",
    "provider": "local",
    "input_tokens": 0,
    "cached_tokens": 0,
    "output_tokens": 0,
    "cost_usd": 0.0,
    "verdict": "pass"
  }
]
```
