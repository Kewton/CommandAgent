# 結果サマリ

- **CM-4主文**: explicit-only `--think`、qwen3.8 medium/high比較、4並行
  headless隔離、R2 bundleと決定的再検証、標準運用runbookを実装・実測した。
  qwen3.8の自動採用は行わず、owner裁定待ちで停止する。
- **規律**: `--think`未指定のqwen3.6 request bytesはfixtureと完全一致し、
  medium宣言時だけrequest fieldが存在する。doctor/summaryは実効値とfield
  presenceを記録する。golden/schema/adversarialの封緘3層は不変。
- **較正**: E mediumはfull 8/12、p50 59秒、F highはfull 8/12、p50
  148.5秒。full率差0pp [Newcombe 95% CI −33.81,+33.81]、Fのp50は
  Eより89.5秒長い。このn=12/armを超えて一般化しない。
- **運用実証**: qwen3.6の4 processはowner path/run ID/tree hashが全件独立、
  foreign path参照0、実効speedup 3.160倍。ただし品質は0/4 fullで、
  process隔離成立と品質成立を混同しない。
- **納品**: L2 full 1件を11-file R2 unitへ封緘し、manifest/binary SHAを
  先行照合後、製品S/ZとPython参照を2回再実行してfull一致。BはL2非適用、
  promotionはevidence claimなしと明示した。
- **費用**: CM-4新規liveはE/F `$0.05470334` + 並行4本 `$0.00901714`
  = `$0.06372048`。CM意思決定窓のpricing確定合計は`$0.49151232`、数値正本が
  ある除外窓を含む既知下限は`$0.54277734`。null窓とlocal電力は推計しない。
- **残件**: E採用可否はowner裁定待ち。既存QUEUEDはL2 verify起動形の一意化と
  global集約/schema v0.2候補。新しい設計床はなし。

# CM Phase 0–4 settlement

## Phase values

| phase | fixed observation | disposition |
|---|---|---|
| 0 Builder Plane | local create full 381.794秒、Luna initial partial 14.194秒、保存recovery full 84.42秒。terminal JSONでartifact/events/stopを機械取得 | headless成立 |
| 1 contract/adversarial | sealed 5類型×initial/re-entry=10/10 fail-closed constraint retention。製品経路もinitial 5 failed + repaired 5 full | known suite 100%; exhaustive claimなし |
| 2 golden | 36/36、one-shot 29/36=80.6% [65.0,90.2]、full 34/36=94.4% [81.9,98.5]、p50 174.5秒、max $0.00252714 | 4線達成、Phase 2 GO |
| 3 tier separation | C Terra 12/12 full、B′ local 11/12、D′ Luna/Luna 7/12。D′の30秒×高full率は不成立 | tier map fixed at n=12/arm |
| 4 operations/generation | E/F各8/12 full、E p50 59秒、F p50 148.5秒。4並行cross-contamination 0。delivery reverify 2/2一致 | E採用はowner裁定待ち |

L2 FullはspecのS/Z/材料検証済みを意味し、runtime smokeはplatform統合の
被覆である。L3/L4 Fullだけが有効promotionとS+Z+Bを含む。

## Tier table

| arm | planner / think | executor | full [Wilson 95% CI] | p50 / p95 | cost total | reading |
|---|---|---|---:|---:|---:|---|
| A | qwen3.6 / omitted | Luna | 10/12=83.3% [55.20,95.30] | 181.5 / 218.5秒 | $0.01611224 | sealed operational baseline |
| B | qwen3.6 / omitted | qwen3.5 local | 9/12=75.0% [46.77,91.11] | 180.5 / 237.8秒 | $0 | first sample |
| B′ | qwen3.6 / omitted | qwen3.5 local | 11/12=91.7% [64.61,98.51] | 157.5 / 210.7秒 | $0 | repeat; not intervention effect |
| C | qwen3.6 / omitted | Terra | 12/12=100% [75.75,100] | 179.0 / 206.55秒 | $0.20217050 | highest observed full, higher cost |
| D | Luna | Luna | 7/12=58.3% [31.95,80.67] | 32.5 / 136.45秒 | $0.10150708 | fast, low full |
| D′ | Luna | Luna | 7/12=58.3% [31.95,80.67] | 31.0 / 101.35秒 | $0.07468386 | transmission fixed; model failures remain |
| E | qwen3.8 / medium | Luna | 8/12=66.7% [39.06,86.19] | 59.0 / 74.8秒 | $0.02128278 | candidate, owner decision pending |
| F | qwen3.8 / high | Luna | 8/12=66.7% [39.06,86.19] | 148.5 / 540.45秒 | $0.03342056 | no full gain over E in this sample |

A is quoted from the earlier instrument generation; E/F are a direct
same-instrument think comparison. No automatic default change is justified by
cross-generation point estimates.

## Floor lineage

Phase 2 repaid ten measurement/product floors without weakening gates:

1. platform schema supply;
2. provider usage/pricing/summary cost wiring;
3. planner transmission of the spec artifact and verifier;
4. correct literal AppSpec plus dependency-free verifier invocation;
5. sandbox-independent campaign binary placement;
6. provider reachability before run 0;
7. complete schema-derived closed vocabulary injection;
8. same-entity computed DAG plus core manifest supply;
9. S/Z/B applicability by artifact level;
10. promotion request changed from guidance to a fail-closed gate.

Phase 3 then gated L3 package materials where diagnosis proved transmission,
while retaining model-attributed failures. Phase 4 found and repaid a
reference-only L2 lockfile applicability mismatch. The four-process probe's
two new stop signatures are model artifact failures; isolation itself required
no repair.

## Sealed three-layer anchors

| layer | outer SHA-256 | status |
|---|---|---|
| golden suites | `4ea74f2fe2687989467a9019c4f72a160d38e77097ea441e6de4a066748dad86` | unchanged |
| AppSpec schema fixture | `6242f3549c8b7eea08dd75067fd7e338e24659b76079d03c6ed5185fa58572c1` | unchanged |
| adversarial fixtures | `792c9696ca86127966810ec4a376a3815c4fb93de4ad2c9d6aa205dad09a2b0b` | unchanged |

CM-4 adds independent arm-suite pins and the R2 bundle manifest; neither
rewrites these three historical seals.

## Cost settlement

The decision-window sum is mechanical and avoids double-counting A, which is a
subset quoted from Phase 2:

| window | cost |
|---|---:|
| Phase 2 accepted golden-008 | $0.04820040 |
| CM-3 matrix-001 new B/C/D | $0.30367758 |
| CM-3 matrix-002 B′/D′ | $0.07468386 |
| CM-3 Terra F-0 smoke | $0.00123000 |
| CM-4 E/F | $0.05470334 |
| CM-4 qwen3.6 four-process probe | $0.00901714 |
| **decision-window total** | **$0.49151232** |

Recorded pre-decision Phase 2 floor windows add `$0.05126502`, giving a
pricing-resolvable known lower bound of `$0.54277734`. Phase 0's historical
summary cost is null, some excluded windows have no usage cost record, and
local-model electricity was not measured. They remain unknown rather than
being invented as zero.

## Product-plan v2.2 material

- Phase 0 text should say headless Builder Plane and recovery continuation are
  measured, not "step-runner wiring in progress".
- Phase 1 text should say fixed profile contract, Rust/reference parity, and
  known adversarial suite 10/10; it must not claim universal attack coverage.
- Phase 2 is GO at the four predeclared lines, with L2 Full semantics visible.
- Phase 3 should retain the whole tier table and n=12/arm uncertainty. Terra was
  12/12 here; a 30-second high-full Luna/Luna arm was not established.
- Phase 4 should describe explicit-only think metadata, campaign preflight,
  four-process isolation, R2 manifest delivery, and deterministic later audit.
- Keep E adoption blank until owner adjudication. Keep L2 verify command
  unification and global aggregation/schema v0.2 in QUEUED.
