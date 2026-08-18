CM-3 — Community golden 3種をplanner/executor階級で分離。A（qwen27→Luna）
10/12 full、B（qwen27→local qwen3.5-9b-mlx）9/12、C（qwen27→Terra）
12/12、D（Luna→Luna）7/12。Wilson 95% CIを全率に併記し、n=12/arm内の
比較に限定。Bは閉語彙3件、Dはpackage欠落3件/spec artifact欠落2件で
fail closed。新規live 36/36完走、provider cost $0.30367758、厳密model ID
drift 0、golden封緘SHA不変。モデル因子のcommunity版分離データとして清算。

