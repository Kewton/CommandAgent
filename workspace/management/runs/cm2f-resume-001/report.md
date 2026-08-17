# 結果サマリ

Ollama復旧後のCM-2f再開campaign。既存CM-2f窓（Ollama未接続12件）は再消費せず、新campaignで再実行した。

## 実測

warikan_001〜003を外部実行許可下で完了。Ollama planner接続は成立し、各runは220秒、210秒、192秒でplanner→executor→自己完結verifyまで到達した。3件とも`commandagent --offline --profile community-mini-app ...`が実行され、`community_profile_violation:community_spec_closed_vocabulary`で失敗した。これは配置/接続床ではなく、生成specの閉語彙違反である。同一停止クラスが3件連続したため、残り33runを開始せず停止した。

| run | status | duration | stop |
|---|---|---:|---|
| warikan_001 | failed | 220s | community_spec_closed_vocabulary |
| warikan_002 | failed | 210s | community_spec_closed_vocabulary |
| warikan_003 | failed | 192s | community_spec_closed_vocabulary |

完了fullは0/3（Wilson 95% CI [0%, 70.8%]）、修復込みfull 0/3（同CI）。所要p50=210s、p95=219s。mochimono/voteは未実施。provider usage/costはcampaign metadataのevents正本に従い、推測値を補っていない。

## 判定

L2成果物生成と自己完結verifyの伝達は実測できたが、閉語彙違反が3連続したためPhase 2 GOは宣言しない。
