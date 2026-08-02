# F-1 Retrospective 001

Status: complete (2026-08-02)

固定済みの等配点と`fail / violation = -w/2`だけを用い、既存judgeを追加せず
repository-managed historyをread-only走査した。これは重み選定ではなく、粗い
得点で十分かを検証する走査であり、結果を見てweightは動かしていない。

## Coverage

| profile | inventory | scannable | final-only | checkpoint-capable | reached | full |
|---|---:|---:|---:|---:|---:|---:|
| nextjs | 78 | 0 | 0 | 0 | 0 | 31 |
| circle | 33 | 33 | 33 | 0 | 30 | 1 |
| cli | 98 | 98 | 98 | 0 | 16 | 2 |
| data | 60 | 60 | 60 | 0 | 13 | 2 |
| fix | 30 | 30 | 6 | 24 | 24 | 1 |
| ingest | 54 | 54 | 54 | 0 | 20 | 4 |
| investigation | 12 | 12 | 0 | 12 | 12 | 0 |

総inventoryは365、run-level走査可能は287、final-onlyは251、checkpoint可能は36。旧Next.js 78 runは
集計表だけが現checkoutに残り、run ID・原子列を復元できないため推測せずgapとした。

## Intermediate → final correlation

| profile | model tier | n | Spearman | rule |
|---|---|---:|---:|---|
| circle | cloud | 0 | hidden (n<5) | minimum_sample_guard |
| circle | local | 0 | hidden (n<5) | minimum_sample_guard |
| cli | cloud | 0 | hidden (n<5) | minimum_sample_guard |
| cli | frontier_reasoning | 0 | hidden (n<5) | minimum_sample_guard |
| cli | local | 0 | hidden (n<5) | minimum_sample_guard |
| data | cloud | 0 | hidden (n<5) | minimum_sample_guard |
| data | local | 0 | hidden (n<5) | minimum_sample_guard |
| fix | local | 24 | 0.0629 | reported |
| ingest | cloud | 0 | hidden (n<5) | minimum_sample_guard |
| ingest | local | 0 | hidden (n<5) | minimum_sample_guard |
| investigation | local | 12 | unavailable (constant input) | constant_score_or_final_outcome |

読み: fix × local ρ=0.0629 (n=24)。constant outcomeの層は相関を捏造せずunavailable、n<5は
裁定どおり非表示にした。この結果からweightは変更しない。

## Full = 100 consistency

run-level原子を復元できたfullは10件で、全10件がscore 100かつ全required atom passだった。
旧Next.jsのaggregate-only full 31件は
run-level原子がないため検算分母へ混ぜていない。

## Read-only and anti-overfitting guards

- historical files: 4393 files
- tree SHA-256 before/after: `13c3428588202c820e3f293c18f137f907ff19d315926f09dc7ceeb60b598e38` / `13c3428588202c820e3f293c18f137f907ff19d315926f09dc7ceeb60b598e38`
- historical mutation: false
- invented checkpoint timestamps: false
- new judges: 0
- weights changed after scan: false
