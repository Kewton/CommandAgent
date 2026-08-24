# ローカル時分割BoN — bon-local-001

## 事前宣言

記録時刻: 2026-08-04T10:07:00+09:00。campaign開始前。

- 対象: Next.js createのBreakout単一goalを6回、empty workspace別に逐次実行。
  executorはlocal `qwen3.6:35b-a3b-coding-nvfp4 / ollama`、plannerはlocal
  `qwen3.6:27b-coding-nvfp4 / ollama`、presetは`profile`。
- 計器: revision `3d2409c8189e68fd79e14a2558c4478754dbc4c4`、binary SHA-256
  `df9160d98c6a63fa300cff1477ff4be6898888f2bd143dc7fd125496e687e127`、suite
  SHA-256 `8d467639c37df8a220a60566a09108249baeec9f62531271e921bc5b56e28b0a`。
- 基準率: 生成済みNext.js bandの同じscenario/executor行`2/11=18.18%`、
  Wilson 95% CI `[5.14%, 47.70%]`。当該bandは移行前窓で、現repositoryから
  source rowを再生成できない制約も継承する。
- Jeffreys posterior `Beta(2.5, 9.5)`による6本のBeta-binomial予測は
  `P(full >= 1)=68.50%`、期待full 1.25本、95%最短連続帯`0..3`。
  旧来の点p二項`約70%`は主判定に使わない。
- 実行窓: Asia/Tokyoの2026-08-04 20:00以降に開始し、単一local accelerator上で
  parallelism 1の時分割実行とする。昼間に前倒ししない。
- sampling: `ollama show`でplanner/executorの既定temperatureはともに0.6。
  trial間で固定し、seedはOllama管理で実効値がeventに露出しない。6本のproduct
  tree SHAが全て相異なることをvariationの受理条件とし、衝突しても差し替えrunはしない。
- 参考値としてrun所要、合計所要、開始/終了時刻、可能な範囲のmacOS電力情報を
  記録する。電力は統計判定へ使わない。

機械可読な事前宣言は`evidence/local-breakout-predeclaration.json`へ固定した。

## 実測

2026-08-04 20:25:57 JSTに夜間窓内でcampaign
`nextjs-breakout-local-bon-20260804-112413`を開始し、単一local acceleratorで
6本を逐次実行した。21:11:16 JSTに完了し、合計所要は2,719秒（45分19秒）、
追加・差し替えtrialは0だった。

| run | 秒 | product exit | final / assurance | full | product tree SHA-256先頭 |
|---|---:|---:|---|---:|---|
| `breakout_local_bon_001` | 466 | 1 | not_checked / partial | 0 | `2ee133afa49a` |
| `breakout_local_bon_002` | 486 | 1 | incomplete / partial | 0 | `ff20cd3b1c77` |
| `breakout_local_bon_003` | 605 | 1 | incomplete / partial | 0 | `deacd548c202` |
| `breakout_local_bon_004` | 399 | 1 | incomplete / partial | 0 | `353095868d1a` |
| `breakout_local_bon_005` | 375 | 0 | full_success / full | 1 | `5a33cb2ae227` |
| `breakout_local_bon_006` | 388 | 1 | incomplete / partial | 0 | `011e502414c4` |

一次`run_stop`を事前宣言どおり`ok=true AND final_acceptance_status=full_success
AND assurance_level=full`で再計算するとfullは`1/6`、`>=1`は成立した。005は
`runtime_acceptance=pass / release_gate=pass / task_status=complete`も同時に保持する。
失敗5本は主にrestart/recoverable-state証跡不足でpartialとなり、削除も救済採用も
していない。

開始・終了ともAC給電、Low Power Mode 0。idle sleepだけを`caffeinate -dimsu`で
防止した。終了直後にplanner/executor両modelがOllama上で100% GPU使用と表示された。
外付け電力量計がないためkWhや電気料金は推定せず、これは参考メモに留める。

## 検算

- 計器pin: revision、suite SHA、built/installed binary SHAは全一致。preflightの
  `cargo test`とrelease buildもexit 0。6本は同一command、goal、executorで、
  workspaceはすべてemptyから開始した。
- 予測突合: 観測full 1本は事前のBeta-binomial 95%帯`0..3`内で、期待1.25本との差は
  -0.25本。`>=1`成立は予測68.50%に対する1回の実現であり、確率自体の検証とは
  言わない。
- sampling: `.anvil/`、evidence、cache、console logを除くproduct tree SHAは6/6で
  相異なり、全treeがnon-emptyだった。固定command・既定temperature 0.6の下で
  成果物の実効variationを確認した。ただしproviderがseed実効値をeventへ出さないため、
  seed独立性の直接証明とはしない。衝突もreplacementも0。
- scrub: run別6/6とcampaign全体がgreen、findings 0。
- 補助`acceptance-sheet.md`だけは、005の`run_stop.status=completed`をfull語彙として
  読まず「assurance: 未完了」と表示した。一次event、summary、campaign metadataは
  fullで一致し、事前full述語への影響はないが、表示投影の不整合として隠さず
  `issue_detected`にする。追加計測はしない。

機械検算値は`evidence/local-breakout-result.json`へ固定した。ローカルBoNはこの6本で
CLOSEする。
