# UAT workflow circle elevated-006

The measurement used installed binary `a8279ce` (the subsequent correction
`dd1e8eb` removes an existence precondition discovered from this run). Runs
were sequential and uninterrupted except for the explicitly recorded model
HTTP 500 in run 3.

| run | investigate run | fix run | elapsed | result |
|---|---|---|---:|---|
| 1 | `019f8aac-5445-7de1-bc54-4e369e2232d2` | `019f8aac-d17e-7eb0-a8e2-dd9fcb19f123` | 35 s | `repair_target_unresolved` |
| 2 | `019f8aad-047b-7570-8f7c-64a20ef276b9` | `019f8aad-4d38-76f3-87aa-025e8fd61c57` | 20 s | `repair_target_unresolved` |
| 3 | `019f8aad-7a6d-70e3-bf15-7c9f6c47c70a` | none | 6 s | investigate diagnose HTTP 500 from Ollama |

Runs 1 and 2 reached the fix node and ended with:

```json
{"event":"workflow_adjudicated","reason":"node_failed:fix","verdict":"circle_failed"}
```

Run 3 ended with `node_failed:investigate`; it was not retried by decision.
No circle_full or verify_origin evidence exists.

## Target-resolution audit

The new `r_command_mapped` and manifest producer rules were not yet in the
installed binary. Consequently runs 1 and 2 still report unresolved targets
for the elev-005 shapes. The correction was then made in `dd1e8eb`: command
paths are accepted as workspace-relative generation targets even when the
file is missing, and all four data catalog checks map through the producer
rule when the pipeline directory exists. Focused tests cover both shapes;
the next measurement is required to assess runtime selection distribution.

## Evidence and safety

The copied run directories contain workflow events/circle records, node
events, I2 binding and fix evidence where produced. Run 1 and 2 I2 bindings
had 6/6 and 5/5 matched claims respectively. All recorded node paths are
origin-confined. Scrub command:

```sh
python3 workspace/management/scripts/bench.py scrub \
  --path workspace/management/runs/uat-test0722-circle-elev-006
# {"ok":true,"findings":[]}
```

Credential-pattern grep over evidence and logs was empty. Raw run logs are
untracked and excluded from the commit.
