# D-3a-3k target-resolution prerequisite audit

| rule | prerequisite | type | treatment |
|---|---|---|---|
| verified_diagnosis_mapped | I2 matched claim and workspace-relative path | context/format | retain match and confinement; no target existence check |
| diagnosis | carried diagnosis text | context | carry only; no filesystem gate |
| traceback | parsed file/line | format | parser and confinement only |
| contract producer | catalog check identity | context | manifest IDs map to `pipeline/main.py` |
| r_command_mapped | path token in R | format | normalize and confine; no file/parent gate |
| evidence | evidence key | context | existing evidence mapping |
| required_path | declared path | format | normalize and confine; no worktree gate |

Existence prerequisites were removed from the resolver. Creation of missing
parents belongs to the write stage. Future prerequisite changes must update
the permanent table in `src/planner/repair_targeting/verified.rs`.
