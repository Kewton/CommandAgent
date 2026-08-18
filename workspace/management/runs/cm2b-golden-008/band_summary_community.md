# Community band summary

Full meaning: L2 `full`はspec検証済み（schema/拘束/材料）であり、runtime
smokeはプラットフォーム統合の被覆である。L3/L4 `full`は有効なpromotionと
S+Z+Bを全件passし、bundleとmanaged runtime smokeを実測済み。本計測は
L2=36、L3/L4=0である。

| 種別 | run | 一発full | 修復込みfull | failed | cost合計 |
|---|---:|---:|---:|---:|---:|
| warikan | 12 | 10 | 12 | 0 | $0.01574440 |
| mochimono | 12 | 9 | 10 | 2 | $0.01711858 |
| vote | 12 | 10 | 12 | 0 | $0.01533742 |
| 全体 | 36 | 29 | 34 | 2 | $0.04820040 |

duration五数要約（秒）: min=139、Q1=161、median=174.5、Q3=194、
max=235。p95=216.25秒。一発full=80.6%（Wilson 95% 65.0–90.2%）、
修復込みfull=94.4%（81.9–98.5%）。4判定線をすべて達成しPhase 2 GO。
