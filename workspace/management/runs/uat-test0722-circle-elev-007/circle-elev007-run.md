# circle-elev-007 runbook

Start condition verified: commit `7dc672e` is installed after release build
and both CI (`29969527047`) and acceptance (`29969527002`) are green. The
binary hash must be checked again immediately after install with
`commandagent --version`; it must include `7dc672e` or a later commit.

Run these exact commands from the repository root, one at a time. Do not
monitor, inspect, interrupt, or start the next command until the prompt
returns. Record both epoch values. If Ollama returns HTTP 500 and the run
stops before node completion, preserve that origin as interrupted and use
one fresh-copy retry only; retain both records.

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev007_origin_1 ; date +%s
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev007_origin_2 ; date +%s
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev007_origin_3 ; date +%s
```

After all prompts return, report completion. Recovery will record selection
reasons and target paths, fix model turns and diffs, I2 statistics,
verify_origin reachability, containment, and scrub results.
