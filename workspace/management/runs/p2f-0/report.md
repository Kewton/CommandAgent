# P2F-0 settlement

> GENERATED FILE: DO NOT EDIT.
> Regenerate: `python3 workspace/management/scripts/p2f_campaign.py settle --recorded-at 2026-08-05T03:18:57+09:00`

## 事前宣言 → 実測 → 検算

### 事前宣言

failed母集団44本をcensusし、失敗クラス×開始スコア帯の9非空セルへ固定seed SHA順位で配分した10本を、原workspaceのcopy上で保存済みrecovery UltraPlanへ各1周だけ通す。先行は円環1/3、Wilson 95% CI 6.1–79.2%。Jeffreys更新Beta-binomialによる10本のfull本数95%予測帯は0..9。層別点予測は置かない。BoN修復接続、自動配線、directive注入は0。

### 実測

P2F@1は **1/10 = 10.0%**。Wilson 95% CI [1.8%, 40.4%]。fix単独総所要 8,971.866秒、API費用 $0.4039273。唯一のfullは`bon0-002r/filter_bon0_001`（未到達→100）。

| # | census id | failure stratum | start band | verdict | score before → after | fix sec | fix cost |
|---:|---|---|---|---|---|---:|---:|
| 1 | `bon-local-001/breakout_local_bon_004` | `nextjs_evidence` | `unreached` | failed | 未到達 → 未到達 | 540.061 | $0.0000000 |
| 2 | `bon0-001/filter_bon0_001` | `cli_claim_binding` | `mid:37.5-<75` | failed | 62.5 → -50 | 1080.179 | $0.0468521 |
| 3 | `bon0-001/filter_bon0_002` | `cli_polarity` | `low:<37.5` | failed | 25 → -37.5 | 1140.233 | $0.0549823 |
| 4 | `bon0-001/filter_bon0_004` | `cli_polarity` | `mid:37.5-<75` | failed | 62.5 → -12.5 | 1320.235 | $0.0568585 |
| 5 | `bon0-002r/filter_bon0_001` | `phase_verification` | `unreached` | full | 未到達 → 100 | 1290.290 | $0.0571709 |
| 6 | `bon0-002r/filter_bon0_006` | `other_acceptance` | `mid:37.5-<75` | failed | 37.5 → -50 | 690.168 | $0.0686214 |
| 7 | `bon0-003r/filter_bon0_002` | `profile_probe` | `unreached` | failed | 未到達 → 未到達 | 240.061 | $0.0143725 |
| 8 | `bon0-003r/filter_bon0_006` | `cli_polarity` | `mid:37.5-<75` | failed | 62.5 → -12.5 | 1140.283 | $0.0439188 |
| 9 | `bon0-004r/filter_bon0_001` | `cli_claim_binding` | `low:<37.5` | failed | 25 → 25 | 990.222 | $0.0446932 |
| 10 | `luna-006/stats_luna_002` | `profile_probe` | `high:75-<100` | failed | 75 → 未到達 | 540.134 | $0.0164576 |

### 検算

観測full本数1は事前Beta-binomial 95%帯0..9の内側。先行33%点との見かけの差を、n=3先行とn=10標本から有意差や定常率へ昇格しない。原workspace tree SHAは10/10前後一致、copyのproduct treeは10/10変化、1周10/10、directiveなし10/10。production/workflow集約SHAと既存7 band byte SHAも事前pin一致。

数値比較可能な6本は改善0、横ばい1、悪化5。nullableな4組は差をゼロへ潰さない。これはfixが変更を作ったことと、受理へ近づいたことが同義でないことを示す。

## 失敗クラス別（記述）

| failure stratum | full/n | after score reached | fix seconds | fix cost |
|---|---:|---:|---:|---:|
| `cli_claim_binding` | 0/2 | 2/2 | 2070.401 | $0.0915453 |
| `cli_polarity` | 0/3 | 3/3 | 3600.751 | $0.1557596 |
| `nextjs_evidence` | 0/1 | 0/1 | 540.061 | $0.0000000 |
| `other_acceptance` | 0/1 | 1/1 | 690.168 | $0.0686214 |
| `phase_verification` | 1/1 | 1/1 | 1290.290 | $0.0571709 |
| `profile_probe` | 0/2 | 0/2 | 780.195 | $0.0308301 |

## 開始スコア帯別（記述）

| starting score band | full/n | after score reached | fix seconds | fix cost |
|---|---:|---:|---:|---:|
| `high:75-<100` | 0/1 | 0/1 | 540.134 | $0.0164576 |
| `low:<37.5` | 0/2 | 2/2 | 2130.455 | $0.0996755 |
| `mid:37.5-<75` | 0/4 | 4/4 | 4230.865 | $0.2162508 |
| `unreached` | 1/3 | 1/3 | 2070.412 | $0.0715434 |

唯一のfullは開始未到達帯1/3。low 0/2、mid 0/4、high 0/1で、開始スコアが高いほどfix成功しやすいという単調性は観測しなかった。n小の記述であり、相関不存在の主張はしない。BoN-3のscore gate解除材料にはならない。

## 為替レート：full 1件の観測調達単価

| route | accounting population | full | observed total sec | observed total cost | sec/full | cost/full |
|---|---|---:|---:|---:|---:|---:|
| BoN new N | 30 trials / 5 instances | 2 | 39,935.000 | $1.3016072 | 19,967.500 | $0.6508036 |
| failed + one fix | 10 trials / 10 instances | 1 | 21,153.866 | $0.7894095 | 21,153.866 | $0.7894095 |
| single reference | 48 trials / 48 instances | 2 | 36,783.000 | $1.2001314 | 18,391.500 | $0.6000657 |

fix行は原failed run 12,182.000秒/$0.3854822と、fix 1周 8,971.866秒/$0.4039273を合算。local 1本のUSD API費用は$0、電力量は未計測。CLI×Lunaだけのfix sliceは1/9、20214.805秒/$0.7894095 per full。

比較は時期・family・抽出条件を揃えた無作為試験ではなく、現有実測の記述的為替表である。BoNは5窓30新規run、singleはCLI Lunaの既存48単発run、fixは層別failed 10本を分母とする。

## 裁定材料

既存fix継続が不合格をfullへ拾う機構は1/10で実測した。一方、自動BoN修復接続のNO-GOは維持する。高開始スコアの優位、修復によるscore単調改善、低単価優位はいずれもこの標本では支持されない。P2F-1（人間指示版）と混ぜず、本campaignはCLOSEする。
