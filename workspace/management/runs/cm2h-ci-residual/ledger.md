# CM-2h CI residual ledger

| item | purpose | final evidence |
|---|---|---|
| CM-2h / CI residual | `15a39ce9`と`2dd3bee1`のred-on-red残務を同一SHAで再実行。先行failureは全て`Set up job`中のGitHub codeload action archive 429/503で、checkout・製品コード・test実行前だったためinfra帰属。 | `15a39ce9` acceptance run `32040625269` attempt 2 `success`; `2dd3bee1` CI run `32041927284` attempt 4 `success`; `2dd3bee1` acceptance run `32041927271` attempt 3 `success`. 最終attemptはいずれも`Repository acceptance suite`まで`success`。 |
