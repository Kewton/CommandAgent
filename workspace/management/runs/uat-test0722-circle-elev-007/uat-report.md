# UAT workflow circle elevated-007

The installed binary was `7dc672e` and both prerequisite workflows were
green before execution (`29969527047` / `29969527002`). Three runs completed
sequentially without HTTP 500 or interruption.

| run | investigate run | fix run | elapsed | circle result |
|---|---|---|---:|---|
| 1 | `019f8c73-1ff5-72f0-8d49-ddf2246de16e` | `019f8c73-3644-7920-bec0-d5f7b71a0a05` | 7 s | `circle_failed`, fix target unresolved |
| 2 | `019f8c73-6e31-7233-8941-082983cd5e75` | `019f8c73-887d-7f63-b7b2-761a2a46afa0` | 9 s | `circle_failed`, fix target unresolved |
| 3 | `019f8c73-bd09-7731-a4f6-7a66a6344390` | `019f8c73-e565-7d92-8ad6-aff16372f3da` | 20 s | `circle_failed`, model read-only stagnation |

All three workflow adjudications were `node_failed:fix`; no circle_full or
verify_origin was reached. I2 binding matched claims were 5/5, 8/8, and 5/5.

## Selection audit

The fix event streams recorded `selection_reason=required_path` (and an
empty selection in the early planner failure), not the expected
`r_command_mapped` or manifest producer mapping. The reason is preserved in
the copied raw evidence: origins 1 and 2 contain `data/` and no `pipeline/`
directory, while the current implementation still required the command-path
parent or pipeline directory to exist before returning a candidate. This is
an implementation gap discovered by the measurement; P1-a (selection
3/3) therefore fails and no successful target-resolution claim is made.
Run 3 reached implementation guidance for `pipeline/main.py` but the model
did not write and hit `model_stagnation:read_only_loop`.

## Evidence and safety

Each run directory contains workflow events/circle records, node events,
investigation bindings, and fix evidence where produced. Node paths are
origin-confined. Scrub:

```sh
python3 workspace/management/scripts/bench.py scrub \
  --path workspace/management/runs/uat-test0722-circle-elev-007
# {"ok":true,"findings":[]}
```

Credential-pattern grep over evidence/logs was empty. Raw run logs remain
untracked and are excluded from the commit.
