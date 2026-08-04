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

夜間窓待ち。

## 検算

夜間窓待ち。
