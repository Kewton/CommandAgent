# Community band summary — Phase 2 settled

Status: **definitive / Phase 2 GO**

Full meaning: L2 `full`はspec検証済み（schema/拘束/材料）であり、runtime
smokeはプラットフォーム統合の被覆である。L3/L4 `full`は有効なpromotionと
S+Z+Bを全件passし、bundleとmanaged runtime smokeを実測済み。本bandの実測は
L2=36、L3/L4=0であり、L2をruntime smoke済みとは表示しない。

| 種別 | run | 一発full | 修復込みfull | failed | duration p50 | cost合計 |
|---|---:|---:|---:|---:|---:|---:|
| warikan | 12 | 10 | 12 | 0 | 173.5秒 | $0.01574440 |
| mochimono | 12 | 9 | 10 | 2 | 178.5秒 | $0.01711858 |
| vote | 12 | 10 | 12 | 0 | 173.5秒 | $0.01533742 |
| 全体 | 36 | 29 | 34 | 2 | 174.5秒 | $0.04820040 |

- duration五数要約（秒）: min=139、Q1=161、median=174.5、Q3=194、max=235。
  p95=216.25秒。
- cost五数要約（USD）: min=0.00086468、Q1=0.00101360、
  median=0.00122860、Q3=0.00147271、max=0.00252714。
- 一発full: 80.6%、Wilson 95% [65.0%, 90.2%]、閾値60%を達成。
- 修復込みfull: 94.4%、Wilson 95% [81.9%, 98.5%]、閾値90%を達成。
- p50 174.5秒は180秒以下、最大cost $0.00252714は$0.067以下。
- CI `32099916047` / acceptance `32099915895`: ともに`success`。

以上をCommunity Mini Apps Phase 2の本設band値として確定する。
