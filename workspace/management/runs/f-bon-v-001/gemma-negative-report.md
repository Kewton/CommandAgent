# gemma負対照 — bon-neg-001

## 事前宣言

記録時刻: 2026-08-04T08:07:30+09:00。campaign開始前。

- 対象: CLI create、`gemma4:31b-cloud / ollama` executor、
  `qwen3.6:27b-coding-nvfp4 / ollama` planner、empty workspace、6本。
- suiteはformal Window B `uat-test0725-cli-elev-004`とprofile、intent、
  preset、workspace mode、budget、planner/provider、executor、2 goalと3+3行列を
  同一にした。追加差分は系列pin用のsuite id、run名、`min_head`、`bon_series`だけ。
- 計器: revision `26d1db81fb4f1c3e7aaf623fc20bf36985d9b959`、binary SHA-256
  `ad191bb86baade66e35da05e02a9c820810dfbd8079bff25f9cf96e7c5afdf6b`、suite
  SHA-256 `ee2a04d77324830d62f70b073bb8ebe205e2a823d432cee55d0b9e0bf83ea2b8`。
- 基準率: formal Window Bのfull `0/6`、`p-hat=0`、Wilson 95% CI
  `[0.0000, 0.3903]`。
- Jeffreys prior後のposteriorは`Beta(0.5, 6.5)`。6本のBeta-binomial予測は
  `P(0 full)=71.45%`、`P(full >= 1)=28.55%`、期待full `0.4286`、
  95%最短連続帯`0..2`。

したがって旧指示の「p=0実測済みにつき期待full=0」は、方法論v2では
母比率既知という意味には採用しない。負対照の機構仮説は「最頻値0、前窓の固執が
再現するか」とし、0本なら支持、1本以上なら決定論的zero説の正直な反証とする。
分散はprovider管理seed/既定temperature、独立workspace、run別provider-turn証跡で
確認する。preflight不一致、構成同一性不成立、scrub失敗なら支出または採用を止める。

機械可読な事前宣言は`evidence/gemma-negative-predeclaration.json`に固定した。

## 実測

campaign `cli-gemma-negative-bon-20260803-231053`を固定計器で6本完了した。
harnessはexit 0、productは6/6がexit 1、scrubは6/6 green、environment
interruptionとharness retryはともに0だった。

| run | goal | 秒 | final / assurance | full | product tree SHA-256先頭 |
|---|---|---:|---|---:|---|
| stats_gemma_neg_001 | stats | 1712 | not_checked / static | 0 | `2c54b7bfda0c` |
| stats_gemma_neg_002 | stats | 376 | not_checked / static | 0 | `f3ee1c170d98` |
| stats_gemma_neg_003 | stats | 361 | not_checked / static | 0 | `1445ca752df6` |
| filter_gemma_neg_001 | filter | 791 | not_checked / static | 0 | `4ed91eb63835` |
| filter_gemma_neg_002 | filter | 1186 | incomplete / failed | 0 | `4caf06018280` |
| filter_gemma_neg_003 | filter | 452 | not_checked / static | 0 | `4484c7404b90` |

実測fullは`0/6`、所要合計4878秒。executor turnは全runで
`gemma4:31b-cloud / ollama`、plannerは固定suiteどおりだった。成果物tree hashは
6/6で相異なり、同じgoal内でもstats 3/3、filter 3/3が相異なった。したがって
「成果物が同一だから0」ではなく、異なる個体を生成しても受理fullへ届かない形である。

seed/temperatureはsuiteから固定値を渡しておらずprovider defaultがrequestごとに
適用されるが、Ollamaのprovider-turn eventは実効値を露出しない。このため実効
seed/temperatureの直接同一性証明はできず、独立workspace・run別turn・成果物hashの
相異を観測根拠とする。この限界を成功扱いへ丸めない。

## 検算

- 計器pin: expected/observed revision、suite SHA、built/installed binary SHAが全一致。
  preflight cargo testとrelease buildはいずれもexit 0。
- full判定: 6つの`run_stop`を`ok=true AND final_acceptance_status=full_success
  AND assurance_level=full`で再計算し、該当0件。
- 予測突合: 観測0は事前Beta-binomial 95%帯`0..2`内かつ最頻値。旧来の
  点`p=0`を既知としたのではなく、不確実性込み予測と整合した。
- 合算: formal Window Bと合わせて`0/12`、Wilson 95% CIは
  `[0.0000, 0.2425]`。更新posterior `Beta(0.5, 12.5)`から次の6本で
  `P(full >= 1)=18.07%`、95%帯`0..1`。従って母比率0の証明ではない。
- band判別: 個体差（成果物6 hash）はあるがacceptance fullは0/6。この構成は
  「分散がない」のではなく「分散を撒いても受理境界へ届かない固執型」と読む。
- campaign全体scrubを再実行し`ok=true / findings=[]`を確認した。

機械検算値は`evidence/gemma-negative-result.json`へ固定した。負対照はこの6本で
CLOSEし、追い計測しない。
