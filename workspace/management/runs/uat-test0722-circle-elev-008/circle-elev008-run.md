# circle-elev-008 runbook

Start condition: `5742189`; CI `29970582443` and acceptance `29970582466`
are green. After release install, verify `which commandagent` and
`commandagent --version` contains `5742189` or later.

Execute each command from the repository root, sequentially and without
monitoring or interruption until the prompt returns:

```sh
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev008_origin_1 ; date +%s
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev008_origin_2 ; date +%s
date +%s && commandagent --workflow workflows/recovery-circle-data-elevated.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/circle_elev008_origin_3 ; date +%s
```

Capture selection reasons, target paths, fix turns/diffs, verify_origin,
I2, containment, and scrub. HTTP 500 is a non-consuming interruption;
preserve it and allow one fresh-copy retry only.
