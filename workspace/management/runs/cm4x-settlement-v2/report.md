# 結果サマリ

- **preflight**: 作業開始親`e4ece796b5827c1e43e759b11420c031b8266d28`はCI
  `32132994172`、acceptance `32132994180`とも`completed/success`。
- **コミット**: `377154a4`で並行nested verifierをcampaign binaryへ固定し4本を
  再実証、`b21c48a2`でEを同一計器のn=36へ拡張。本清算コミットでCM段v2を固定する。
- **並行4件の帰属**: schema版違反3件の一次原因はmachine/stale binary。
  うち1件は誤repair後にmodelの二次違反を持つ。core manifest path 1件は、正しい
  供給に対するmodelのmalformed Read path。推測補修はしていない。
- **並行再実証**: 同じbinaryと封緘warikan suiteで4/4 exit 0、runtime pass、
  final acceptance `full_success`、release gate pass。交差汚染0、実効speedup 3.297倍。
  generic headless verdictは4件とも`completion_contract_not_bound`によるpartialのまま。
- **planner裁定材料**: qwen3.8 medium E36はone-shot 18/36=50.0%
  [34.47,65.53]、修復込み24/36=66.7% [50.33,79.79]、p50/p95
  61.5/129.5秒。Aとの差は−16.67pp [Newcombe −36.92,+14.38]で0を含む。
  採用は`owner_adjudication_pending`、既定を変更しない。
- **検証・封緘**: local full suite 2,103 pass / 0 fail / 34 ignored、format、
  Clippy、focused Python 44/44、Ruff、scrubがgreen。golden/schema/adversarialの
  外側SHAは不変で、内側全entryも一致。
- **費用**: CM-4/4x新規live `$0.11986258`、CM意思決定窓合計`$0.54765442`、
  pricing解決可能な除外窓込み既知下限`$0.59891944`。null窓とlocal電力は推計しない。
- **未実施・逸脱・新規床**: E採用判断はowner裁定待ち。支出前に検出した同revision
  rebuild SHA driftは0run/$0で除外し、封緘binary実物の再照合で返済。未返済QUEUEDは
  L2 verify起動形とglobal集約/schema v0.2候補の2件。新しい未返済床はなし。

# CM Phase 0–4 settlement v2

## 1. 並行4件の最終帰属

三点対照の正本は`cm4x-parallel-attribution-001/evidence.json`である。全workspaceへ
供給されたschemaは`community.app-spec/v0.1`、schema SHAは`80e4cb41…7ac7e0b`、
core manifest SHAは`f23e87be…e0af`で一致した。契約どおり生成spec rootに
`schema_version`は書かない。外側campaign binaryはversion `26b4705a`、SHA
`03159d12…dbfa`でv0.1を期待したが、計画内の裸の`commandagent`はPATH上の旧
version `178e09c2`、SHA`87861649…784e`へ解決し、旧`community.app-spec/v1`を
期待していた。

| run | 供給 / 生成 / verifier期待 | classification |
|---|---|---|
| warikan_001 | v0.1 / root版記載なし / nested v1 | machine: stale nested verifier |
| warikan_002 | v0.1 / 初回なし→誤repairでv0.1追記 / nested v1 | machine一次 + model repair二次 |
| warikan_003 | v0.1 / root版記載なし / nested v1 | machine: stale nested verifier |
| warikan_004 | 正規`core.sha256sums`供給 / spec生成前 / 正規path期待 | model: Read pathへ自然言語断片混入 |

旧binaryはv0.1を`community_schema_version_invalid`で拒否し、旧v1 fixtureはschema
gateを通って次の`community_build_inputs_missing`へ進んだ。対してcampaign binaryは
001/003の保存specをfullと再導出したため、binary世代差が原因であることを機械的に
再現している。

返済はparallel probeの子process PATH先頭をcampaign binary directoryへ固定し、
`which commandagent`とSHA一致を実行前に要求するもの。stale PATH負例はfail closed。
同一binary/SHAと封緘suiteで再実行した結果は次のとおり。

| quality / isolation | observed |
|---|---:|
| exit 0 / runtime pass / final acceptance full_success / release pass | 4/4 |
| nested binary SHA matches campaign binary | 4/4 |
| workspace・state path・run ID・summary ownerの一意性 | 全項目pass |
| foreign path reference / cross contamination | 0 / 0 |
| makespan / individual p50 / p95 | 635.85 / 520.50 / 621.81秒 |
| sequential-observed比のeffective speedup | 3.297倍 |
| provider cost | $0.00507942 |

generic headless summaryはCommunity profileのfinal acceptance projectionと別契約で、
4本とも`completion_contract_not_bound`のためpartial。この語をfullへ読み替えず、
品質主張は一次`run_stop`のfinal acceptance / release gateをラベル付きで行う。

## 2. Phase 0–4確定値

| phase | fixed observation | disposition |
|---|---|---|
| 0 Builder Plane | local create full 381.794秒、Luna initial partial 14.194秒、保存recovery full 84.42秒。terminal JSONからartifact/events/stopを機械取得 | headless成立 |
| 1 contract/adversarial | 封緘5類型×initial/re-entry=10/10 fail closed。製品経路もinitial 5 failed + repaired 5 full | known suite 100%; 網羅証明なし |
| 2 golden | 36/36、one-shot 29/36=80.6% [65.0,90.2]、full 34/36=94.4% [81.9,98.5]、p50 174.5秒、max $0.00252714 | 4線達成、Phase 2 GO |
| 3 tier separation | C Terra 12/12 full、B′ local 11/12、D′ Luna/Luna 7/12。30秒×高full率は不成立 | n=12/arm地図 |
| 4 operations/generation | think explicit-only、E36 full 24/36・p50 61.5秒。並行rerunはfinal acceptance 4/4・汚染0。delivery reverify 2/2一致 | E採用はowner裁定待ち |

L2 FullはspecのS/Z/材料検証済みを意味し、runtime smokeはplatform統合の被覆。
L3/L4 Fullだけが有効promotionとS+Z+Bを含む。

## 3. ティア表最終版

| arm | planner / think | executor | full [Wilson 95% CI] | p50 / p95 | cost total | reading |
|---|---|---|---:|---:|---:|---|
| A | qwen3.6 / omitted | Luna | 10/12=83.3% [55.20,95.30] | 181.5 / 218.5秒 | $0.01611224 | sealed baseline |
| B | qwen3.6 / omitted | qwen3.5 local | 9/12=75.0% [46.77,91.11] | 180.5 / 237.8秒 | $0 | first sample |
| B′ | qwen3.6 / omitted | qwen3.5 local | 11/12=91.7% [64.61,98.51] | 157.5 / 210.7秒 | $0 | repeat; intervention効果ではない |
| C | qwen3.6 / omitted | Terra | 12/12=100% [75.75,100] | 179.0 / 206.55秒 | $0.20217050 | highest observed full, higher cost |
| D | Luna | Luna | 7/12=58.3% [31.95,80.67] | 32.5 / 136.45秒 | $0.10150708 | fast, low full |
| D′ | Luna | Luna | 7/12=58.3% [31.95,80.67] | 31.0 / 101.35秒 | $0.07468386 | transmission fixed; model failures remain |
| **E36** | **qwen3.8 / medium** | **Luna** | **24/36=66.7% [50.33,79.79]** | **61.5 / 129.5秒** | **$0.07234546** | **owner decision pending** |
| F | qwen3.8 / high | Luna | 8/12=66.7% [39.06,86.19] | 148.5 / 540.45秒 | $0.03342056 | medium比full gainなし |

A→E36のfull率差は−16.67pp [Newcombe 95% CI −36.92,+14.38]、one-shot差は
−25.00pp [−47.37,+7.22]。Aは前計器世代、E36でもA側はn=12であり、断定は
「この分母では」に限定する。qwen3.6/think omittedの運用既定を自動変更しない。

## 4. 床系譜

Phase 2は次の10系統をgateを弱めず返済した。

1. platform schema供給;
2. usage/pricing/summary cost配線;
3. plannerへのspec artifact/verifier伝達;
4. 正しいAppSpec字義例と依存不要verify;
5. sandbox非依存binary配置;
6. provider到達性0runゲート;
7. schema由来の完全閉語彙注入;
8. same-entity computed DAGとcore manifest供給;
9. 成果物level別S/Z/B適用性;
10. promotionをお願いからfail-closed gateへ変更。

Phase 3は診断が伝達を示したL3 package材料だけをgate化し、model失敗を保持した。
Phase 4はreference-only L2 lockfile適用性を返済した。CM-4xはparallel setupの
**nested verifier binary世代不一致**を追加で返済し、4本を品質込みで再実証した。
E拡張では同revisionの再build SHA driftを支出前に遮断し、既存封緘binary実物を
再照合して同一計器を維持した。これは測定計器の返済であり、新binaryを同一系列へ
黙って混ぜていない。

未返済QUEUEDは、L2 verify起動形の一意化と、global集約を表現するschema v0.2候補。
後者はE拡張でも`luggageItem`のcollection参照として再観測した。

## 5. 封緘3層

| layer | outer SHA-256 | inner check |
|---|---|---|
| golden suites | `4ea74f2fe2687989467a9019c4f72a160d38e77097ea441e6de4a066748dad86` | 3/3 OK |
| AppSpec schema fixture | `6242f3549c8b7eea08dd75067fd7e338e24659b76079d03c6ed5185fa58572c1` | 7/7 OK |
| adversarial fixtures | `792c9696ca86127966810ec4a376a3815c4fb93de4ad2c9d6aa205dad09a2b0b` | 21/21 OK |

追加suiteとrun evidenceは別SHAで封緘し、歴史的3層のbytesを変更していない。

## 6. 費用清算

| window | cost |
|---|---:|
| Phase 2 accepted golden-008 | $0.04820040 |
| CM-3 matrix-001 new B/C/D | $0.30367758 |
| CM-3 matrix-002 B′/D′ | $0.07468386 |
| CM-3 Terra F-0 smoke | $0.00123000 |
| CM-4 E/F | $0.05470334 |
| CM-4 initial qwen3.6 parallel probe | $0.00901714 |
| CM-4x corrected parallel rerun | $0.00507942 |
| CM-4x E extension 24 | $0.05106268 |
| **decision-window total** | **$0.54765442** |

数値正本のあるpre-decision Phase 2床窓`$0.05126502`を加えた既知下限は
`$0.59891944`。Phase 0のnull cost、usage記録のない除外窓、local model電力は
不明のまま保持し、0や推計値を発明しない。

## 7. 企画書v2.2更新材料

- Phase 0はheadless Builder Planeと保存recoveryを実測済み。「step-runner配線中」
  や「XML閾値後にnative追加」ではなく、runner分解後・Responses/native済みと書く。
- Phase 1はfixed profile contract、Rust/reference parity、known adversarial 10/10。
  universal attack coverageとは書かない。
- Phase 2は事前4線を達成したGO。L2 Full meaningを明示しruntime smoke範囲を隠さない。
- Phase 3は全tierとn=12/armの不確実性を保持。Terra 12/12、Luna/Lunaの
  30秒×高full率不成立を同時に記す。
- Phase 4のparallel記述は「初回quality 0/4」から、stale nested verifierで
  measurement-invalidだったことと、返済後final acceptance 4/4・隔離0へ更新する。
  generic headless partialラベルは別契約として併記する。
- qwen3.8 mediumはn=36でfull 66.7%、p50 61.5秒。A差CIは0を含むため、採用欄は
  owner裁定まで空欄とし、現行qwen3.6既定を維持する。
- R2 manifest、決定的再検証、explicit-only think、0run provider gate、使い捨て
  workspace lifecycleをBuilder Plane運用要件へ置く。
- QUEUEDはL2 verifier起動形とglobal aggregation/schema v0.2の2件を維持する。
