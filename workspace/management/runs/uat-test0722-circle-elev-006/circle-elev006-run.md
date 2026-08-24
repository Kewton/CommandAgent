# circle-elev-006 runbook

Implementation under test: `a8279ce`. Run these exact commands from the
repository root, sequentially. Do not monitor or interrupt a command; wait
for the prompt to return before starting the next one.

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev006_origin_1 ; date +%s
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev006_origin_2 ; date +%s
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev006_origin_3 ; date +%s
```

The audit must capture `selection_reason`, fix model turns, post-repair
verification failures, and the complete workflow adjudication. Scrub all
collected evidence before committing; raw run logs stay untracked.
