# B-2d primary-source investigation

Date: 2026-07-14

Campaign: `uat-test0714-m4-001`

Source workspace: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0714_m4_001`

This note preserves the primary-source findings used to scope B-2d. It is the
repository-managed equivalent of `workspace/temp/a.md` and adds the Run 1
traceback evidence used for DATA-9.

## DATA-7 — verify lint rejection

Run 3 and Run 4 contain six planner lint rejections with the shared reason
`verify command may not use shell control syntax`. Three representative event
pairs are:

| Run / attempt | Planner response | Rejection |
| --- | --- | --- |
| Run 3 / 1 | `artifacts/data_agg_qwen27_plan_qwen35_exec_preset_none_001/events.jsonl:130` | same file, line 131 |
| Run 3 / 2 | same file, line 133 | same file, line 136 |
| Run 4 / 1 | `artifacts/data_agg_qwen27_plan_gemma31_exec_preset_none_001/events.jsonl:13` | same file, line 14 |

The rejected command itself is not recoverable from those artifacts:

- the lint event retained `err.to_string()` but not the offending command;
- `model_response_raw` retained only the first 500 characters;
- the Run 3 recovery document recorded `Verification commands: none` at
  `anvil-repairs/repair-phase-calculate-aggregations-and-totals-019f5f32-74c3-70d2-8742-0d6fde7791b8.md:22`.

Run 3 attempt 2 does retain a separate successful canonicalization:
`python verify-results.py` became
`anvil-catalog-check:data_results_schema` and
`anvil-catalog-check:data_reconciliation` (`events.jsonl:134`). It is not the
rejected command and must not be used as a guessed rewrite fixture.

Conclusion: DATA-7 must first retain `step_id`, `command_index`,
`original_command`, attempted `normalized_commands`, and `violation_kind`.
Rewrite rules remain deferred until a later measurement supplies real command
text.

## DATA-8 — hidden `.anvil` plan inspection

The initially blocked Bash command was:

```bash
cat /Users/<user>/share/work/localwork/commandagent_mvp/01/test0714_m4_001/data_agg_qwen27_plan_qwen35_exec_preset_profile_002/.anvil/* 2>/dev/null || echo "No .anvil files"
```

It was rejected as `verify_command_policy_error / shell_control_syntax`, not as
a hidden-path access (`artifacts/data_agg_qwen27_plan_qwen35_exec_preset_profile_002/events.jsonl:42`).
The following repair turn then allowed all three Bash listings, including the
private directory (`events.jsonl:47`):

```text
ls -la <workspace>/
ls -la <workspace>/data/
ls -la <workspace>/.anvil/plans/
```

The final model turn issued these native calls in order (`events.jsonl:58`):

```text
Read data/sales.csv
Read .anvil/plans/plan-019f5f61-406c-7440-8147-748b676a0583.yaml
Read .anvil/plans/ultra-plan-019f5f5f-1348-7b53-8886-2acab70871c7.yaml
```

The data read succeeded. The first plan read ended the phase with
`workspace_policy_blocked: path component .anvil is hidden from normal tasks`
(`events.jsonl:61`); the third call was never executed.

This was ordinary context inspection, not attempted evidence fabrication:
the target was `.anvil/plans`, there were no Write/Edit calls, changed paths
were empty, and no evidence or output JSON was fabricated. DATA-8 therefore
uses uniform engine-private denial plus deterministic bounded continuation
feedback, rather than adding an evidence-fraud contract rule.

## DATA-9 — Run 1 Python traceback

Run 1 executed `python3 -B pipeline/main.py` and captured the following
untruncated stderr in
`artifacts/data_agg_qwen27_plan_qwen35_exec_preset_profile_001/evidence/pipeline-run.json:20`:

```text
Traceback (most recent call last):
  File ".../pipeline/main.py", line 169, in <module>
    run()
  File ".../pipeline/main.py", line 89, in run
    amount = parse_amount(row["amount"])
  File ".../pipeline/main.py", line 53, in parse_amount
    return int(val.strip())
ValueError: invalid literal for int() with base 10: ''
```

Before B-2d, the step failure and repair prompt retained only
`pipeline_probe:pipeline_exit_nonzero` (`events.jsonl:93`). The repair model
then repeatedly read `pipeline/main.py` (`events.jsonl:97-105`) and exhausted
the read-only interlock without receiving the final frame, exception type, or
message.

Conclusion: the deterministic repair target is `pipeline/main.py`, selected
from the final traceback frame with `selection_reason=traceback_mapped`. The
parser must also emit a status-bearing `pipeline_error_extraction` fallback
when a Python failure does not contain a parseable traceback.

## Scope decisions

- DATA-7 adds observability and feedback only; no lint relaxation or command
  rewrite was admitted.
- DATA-8 makes `.anvil` uniformly private to task tools and routes denial into
  the existing bounded read-only pressure discipline.
- DATA-9 parses Python runtime failures, injects deterministic repair context,
  and resolves the mapped source through the shared repair-target selection
  boundary.
