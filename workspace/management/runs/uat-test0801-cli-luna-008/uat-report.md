# uat-test0801-cli-luna-008: 床3枚返済後のLuna安定値札

実施日: 2026-08-02 (JST)

裁定契約: `docs/cli-profile-contract.md` (fixed 2026-07-24)

計測revision: `612b20324d1a8bcb854bd4a771d2299b8d69f8cb` (`develop`)

## 1. 結論

**床3枚返済後のLuna-008はfull 1/6、C到達3/6、到達runのC3は
pass 2 / violation 0 / claims_absent 1だった。自動分類はknown 5 / success 1 /
UNKNOWN 0である。**

Luna-007のfull 1/6とC3 pass 2を別の6runで再現した。fullは
`stats_luna_002`で成立し、C1〜C4が全pass、README claim 1/1が実stdoutと
一致した。C3に到達した他2runは、`stats_luna_001`がclaim 1/1一致、
`filter_luna_003`がclaims_absentだった。C3 violationは0だった。

床返済のlive探針では、成功runがfailure分類を通らず`success`へ入り、
UNKNOWN 0を得た。`stats_luna_003`のverifyは必須positional
`data/sample.csv`を保持したまま実行され、Luna-007の脱落形は再発しなかった。
ただし列名`amount`が実sampleの`id,value,score`と不一致だったため、C到達前に
正直に失敗した。C1字義ガイダンスはmanifest/scaffoldの双方へ固定済みだが、
モデルがREADME先頭usageを裸metavarに戻した2件はC1で引き続き拒否した。

## 2. Campaign境界

- campaign: `cli-create-luna-20260802-082157`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0801_cli_luna8`
- suite: `cli-create-luna`, `profile=cli`, `intent=create`,
  `workspace_mode=empty`, `api=responses`, `tool_protocol=native`
- planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- executor: `gpt-5.6-luna` / `openai`
- run matrix: stats×3、filter×3
- product run retry: 0
- product terminal再実行: 0
- same-campaign resume: 1（4本目と5本目の間。完了済み4本はskip）

既知の未追跡`history.txt` 2件と、既存の未追跡`.agents/` / `.claude/`は移動・
削除・stashせず、同一HEADのdetached clean worktreeからbenchを実行した。
実行セッション境界後は同じcampaignを`--resume`し、完了済みproduct terminalを
再実行していない。前半4本と後半2本の埋込みbuild commitはともに
`612b2032`、`build_dirty=false`。resume preflightはgit clean、minimum ancestor
`527bdc1e`、`cargo test`、release buildがgreenだった。built/installed binary
SHA-256はともに
`ab361951d7dab14f7e249ff416d3177ba9dc68d8a283b3ed9c76b7d82898607a`、
versionは`commandagent 0.1.0 612b2032 2026-08-02T09:42:11Z`だった。

空の除外directory `cli-create-luna-20260802-081417`にはcampaign metadata、run、
artifactがなく、分母・費用・retryへ算入しない。

## 3. Run行列

| run | family | verdict | assurance | C1 | C2 | C3 | C4 | 停止主因 | 秒 | reasoning | 費用 |
|---|---|---|---|---|---|---|---|---|---:|---:|---:|
| `stats_luna_001` | stats | failed | failed (`cli_assurance_failed`) | fail | pass | **pass** | pass | C1 bare metavar polarity | 1,478 | 884 | $0.043928 |
| `stats_luna_002` | stats | **complete** | **full** | pass | pass | **pass** | pass | completed | 1,316 | 1,901 | $0.043806 |
| `stats_luna_003` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | verify column mismatch; positional retained | 663 | 594 | $0.022552 |
| `filter_luna_001` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | README未生成 | 396 | 42 | $0.001423 |
| `filter_luna_002` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | C1 case extraction (`README.md:8`) | 1,379 | 925 | $0.055887 |
| `filter_luna_003` | filter | failed | failed (`cli_assurance_failed`) | fail | pass | claims_absent | pass | C1 bare metavar polarity | 1,366 | 1,242 | $0.050592 |

全runのharness statusは`completed`。product exitは0が1件、1が5件。C3分布は
pass 2 / violation 0 / claims_absent 1 / not reached 3。投影分布はfull 1 / failed 2 /
static 3で、契約§4との不整合は0/6だった。

## 4. C3実物監査

### `stats_luna_002` — full成立

README claim (`README.md:22->23`)と実出力:

```text
python cli/main.py data/sample.csv --column price
Count: 3
Sum: 300
Average: 100
```

claim 1/1は字義一致。C1の凍結正常caseは
`python cli/main.py data/sample.csv --column price`でexit 0、invalid
`--anvil-invalid-probe`はexit 2、rerunはexit/stdout/stderr一致だった。C2はhelpの
3 optionsと未知option拒否を全passした。自動分類は`success / not_applicable`で、
full成功をfailure語彙へ流さない新しい成功区分をliveで確認した。

### `stats_luna_001` — C3は正直、C1だけ不成立

README claim (`README.md:22->23`)と実出力:

```text
python cli/main.py data/sample.csv --column price
count: 3
sum: 350
average: 116.667
```

claim 1/1は一致。一方、README先頭usageから凍結したC1正常caseは
`python cli/main.py INPUT_CSV --column COLUMN`で、存在しない`INPUT_CSV`により
exit 1。invalidはexit 2、rerunは同一、C2/C4はpassのため、C1違反だけを理由に
failedへ投影した。

### `filter_luna_003` — claims absent

README先頭usageは`python cli/main.py --pattern PATTERN [--count] FILE`で、C1は
存在しない`FILE`によりfail。READMEには実行commandと出力blockが別々に置かれ、
C3の隣接claimとして抽出できなかったため`claims_absent`。C2/C4はpassであり、
C1違反を保持してfailedへ投影した。

## 5. C未到達3件と床探針

- `stats_luna_003`はplanner verify
  `python cli/main.py --column amount data/sample.csv`をそのまま実行した。
  必須positional `data/sample.csv`は保持され、Luna-007実形の「positional脱落」は
  0件。ただし実sample列は`id,value,score`で、`amount`は存在せずexit 2となった。
  自動表示classは既存stop vocabularyにより
  `cli_verify:canonical_command_dropped_positional_input`だが、live bytesの監査では
  positional脱落ではなく列値不一致である。
- `filter_luna_001`は`README.md`を生成せずimplement phaseで停止。
- `filter_luna_002`はREADME 8行目を裸metavarで記述し、
  `case_extraction_failed: README.md:8`で正直終端した。

DATA-1定形はmanifest guidanceとpreset scaffoldの双方に、具体例
`python3 cli/main.py data/sample.csv --column amount`と「`<...>` placeholderは
実行argvへ束縛不能」という1行を固定している。fixtureはLuna-007
`stats_luna_003/README.md:8`の実形を入力に、この2経路への字義例配布を検証する。
本窓は配布後のモデル追従率を測るもので、裸metavarをsampleへ推測置換して
合格させてはいない。

## 6. Gemma基準線・Luna窓推移

| arm | denominator | full | C到達 | C3判定 | 合計秒 | 費用 |
|---|---:|---:|---:|---|---:|---:|
| Gemma正式Window B (`elev-004`) | 6 | 0/6 | 2/6 | README捏造6件をviolation拒否 | 3,739 | 記録なし |
| Luna 001 | 6 | 0/6 | 0/6 | 未判定・machine BLOCKED | 2,571 | $0.000000 |
| Luna 002 | 6 | 0/6 | 0/6 | 未判定・machine BLOCKED | 2,488 | $0.000000 |
| Luna 003 (`text`) | 6 | 0/6 | 0/6 | 未判定 | 2,123 | $0.038459 |
| Luna 004 (`text`+自己記録) | 6 | 0/6 | 0/6 | 未判定 | 2,598 | $0.118284 |
| Luna 005 (`text`+dialect repair) | 6 | 0/6 | 0/6 | 未判定 | 3,069 | $0.202160 |
| Luna 006 (`responses`+native) | 6 | 0/6 | 5/6 | pass 2 / violation 1 / absent 2 | 8,655 | $0.312987 |
| Luna 007 (`responses`+native) | 6 | 1/6 | 2/6 | pass 2 / violation 0 / absent 0 | 8,681 | $0.310053 |
| **Luna 008（床返済後）** | **6** | **1/6** | **3/6** | **pass 2 / violation 0 / absent 1** | **6,598** | **$0.218188** |

Luna合算n=48はfull 2/48、C到達10/48、費用$1.200131。001/002はmachine
BLOCKED、003〜005はtext bridge、006〜008はResponses/native窓として区分する。
Responses/native 3窓のC3合計はpass 6 / violation 1 / claims_absent 3、fullは
2/18である。007→008はfull 1/6とC3 pass 2を再現し、full率とC3 pass数の
安定値札を得た。

## 7. Responses/nativeとドリフト探針

| run | provider turns | native tool calls | input | cached input | output | reasoning |
|---|---:|---:|---:|---:|---:|---:|
| `stats_luna_001` | 16 | 21 | 102,921 | 91,114 | 3,835 | 884 |
| `stats_luna_002` | 17 | 19 | 102,026 | 88,962 | 3,641 | 1,901 |
| `stats_luna_003` | 10 | 10 | 34,646 | 27,971 | 2,180 | 594 |
| `filter_luna_001` | 1 | 2 | 805 | 0 | 103 | 42 |
| `filter_luna_002` | 20 | 20 | 143,901 | 129,447 | 4,748 | 925 |
| `filter_luna_003` | 14 | 19 | 85,104 | 71,527 | 4,977 | 1,242 |
| 合計 | **78** | **91** | **469,403** | **409,021** | **19,484** | **5,588** |

78/78 turnsでrequested/returned modelは`gpt-5.6-luna`、service tierは
`default`、`api=responses`、native tools enabled。system fingerprintはprovider
未提供の`null` 78/78で、版同一性を積極的には証明しない。response IDは78/78で
相異なる。78 responsesすべてが1件以上のnative function callを返し、合計91 calls。
endpoint rejectionとtext parse failureは0だった。

## 8. コストと時間

provider turn eventのreturned usageを合計し、系列で固定した2026-08-02確認済みの
Luna単価（uncached input $1.00/M、cached input $0.10/M、output $6.00/M）を適用した。

- uncached input: 60,382 tokens = $0.060382
- cached input: 409,021 tokens = $0.040902
- output: 19,484 tokens = $0.116904
- campaign計: **$0.218188**
- reasoning: 5,588 tokens（output内数）
- run開始: epoch `1785659067`
- resume preflight開始: epoch `1785663579`
- resume preflight終了: epoch `1785663758`
- run終了: epoch `1785666503`
- run合計: 6,598秒
- 最初のrun開始→run終了: 7,436秒（同一campaign resume境界を含む）

## 9. E-0検収とscrub

- 自動分類: known 5 / success 1 / UNKNOWN 0
- 自動検収シート: 6/6
- calibration collector候補: 0（C2/C3 nearest_missなし）
- run別scrub: 6/6 green、findings 0
- campaign scrub再実行: green、findings 0
- report scrub: green、findings 0
- `OPENAI_API_KEY`実値のexact scan: campaignとrepo保存report/summaryのmatches 0
- `.env`は読取り元に使っただけで変更・commitしていない

## 10. 合否

- P0-a 6/6正直終端: **pass**
- P0-b 契約§4投影: **pass**
- P0-c 偽成功ゼロ: **pass**
- 資格情報scrub: **pass**
- Responses/native transport: **pass**（78/78 turns、91 tool calls）
- 既知positional脱落の再発: **0件**
- success分類: **1件、UNKNOWN 0**
- 記録値 full: **1/6**
- 記録値 C到達: **3/6**
- 記録値 C3: **pass 2 / violation 0 / claims_absent 1**
- 記録値 OpenAI費用: **$0.218188**

## 11. 一次資料SHA-256

- `stats_luna_001/cli-case-binding.json`:
  `c1fdb7e5648bcd5540ced923a9886227c9afb6a777568840545b069c3e7cb798`
- `stats_luna_001/cli-probe.json`:
  `0753841e3d74387d8fae17998b6cf59621abe575445fd34db951495799a20fbe`
- `stats_luna_001/help-binding.json`:
  `f3bd5c638e62fc1ca98d9079938db208c93c49c127db266acc5b1e8a81632f66`
- `stats_luna_001/cli-assurance.json`:
  `a3d2ec9bba1aa0d9e0ad83e1f8e73705c6a555d229972b41d81ae7d786584c6f`
- `stats_luna_002/cli-case-binding.json`:
  `49050e394593fedd20910250e756f472b4d64eeff31cb87f944df1b76749cacb`
- `stats_luna_002/cli-probe.json`:
  `bbdc0fe5f7e972acbe876fd99806c4b4db852cb1cbdd9cec4dd30e2a9550b3f0`
- `stats_luna_002/help-binding.json`:
  `81710897fb3fa599cf472ad6e23bcfbbff9ddb6faaad8b9b310c3cddd2c7133a`
- `stats_luna_002/cli-assurance.json`:
  `f6900270e1039afe7ee6fe7ab1300f774178e22439a6860ffbc58732b87b8b2b`
- `filter_luna_003/cli-case-binding.json`:
  `bbb1270f84b099d01206a1adeea8a6d79a9681d8f56eed28a51b2a7ba30e6868`
- `filter_luna_003/cli-probe.json`:
  `c452ce8dff57f586ad50223e233511046fb658ed3c3f083936d79dfa7a5c572d`
- `filter_luna_003/help-binding.json`:
  `954922ebaf9021e3ec0851dd850535e8370691c9b3bd5c93c79eb0573a598b0c`
- `filter_luna_003/cli-assurance.json`:
  `93ee1bfa2d11de482ffa0c9eab1bb28b8376dc2263f703e519da7109be226218`

## 12. Repository verification

- campaign preflight `cargo test`: green
- campaign preflight release build / installed `--version`: green
- commit前の`cargo fmt --all -- --check`: green
- commit前の`cargo clippy --all-targets -- -D warnings`: green
- commit前の`cargo test`: green
- Python `unittest discover`: green
- Ruff: green
- corpus regression、classes双方向guard、既存非CLI byte fixtureを含めてgreen
