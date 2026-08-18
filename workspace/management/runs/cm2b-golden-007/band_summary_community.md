# Community band summary

Full meaning: L2 `full`はspec検証済み（schema/拘束/材料）であり、runtime
smokeはプラットフォーム統合の被覆である。L3/L4 `full`はS+Z+Bを全件pass
してbundleとmanaged runtime smokeを実測済み。failed・環境中断・未実施は
fullではないが、環境中断と未実施は計画分母へ失敗として混入しない。

| 種別 | 完了run | full | failed | 環境中断（非消費） | 未起動 | cost観測 |
|---|---:|---:|---:|---:|---:|---:|
| warikan | 2 | 0 | 2 | 1 | 9 | $0.00934030 |
| mochimono | 0 | 0 | 0 | 0 | 12 | — |
| vote | 0 | 0 | 0 | 0 | 12 | — |

五数要約（完了run duration秒）: min=388、Q1=500.25、median=612.5、
Q3=724.75、max=837。p95=814.55秒。
