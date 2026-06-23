# Phase30 Focused Worklist

Date: 2026-06-23 JST

Status: completed / closed_excluded

## Focused Proof Policy

Phase30 does not automatically require model-facing focused eval because it is
a decision phase. Focused fixtures or targeted runtime tests are required only
if C49 or C50 is adopted or split forward with a new deterministic behavior.

## Work Items

| item | row | trigger | proof |
| --- | --- | --- | --- |
| F30-C49-DECISION | C49 | Completed. | Decision record plus coverage update. |
| F30-C49-TEST | C49 | Only if a deterministic quality classifier is adopted. | Targeted unit test or focused fixture proving the new classifier field. |
| F30-C49-SPLIT | C49 | Only if a failed proof shows a narrower quality-classification gap. | Failed proof root, downstream phase, owner, and closure condition. |
| F30-C50-DECISION | C50 | Completed. | Decision record plus coverage update. |
| F30-C50-TEST | C50 | Only if a CommandAgent-native CLI/slash helper is adopted. | Targeted CLI/slash parser/help/report test. |
| F30-C50-SPLIT | C50 | Only if a failed proof shows a narrower slash/plan/command gap. | Failed proof root, downstream phase, owner, and closure condition. |

No adopted or split-forward behavior was identified in Phase30, so no focused
fixtures are required.

## Candidate Commands

Use only the commands needed by the final decision:

```bash
git diff --check
```

If eval report parsing is touched:

```bash
python3 tests/test_eval_report.py
```

If C50 parser behavior is touched, select the smallest matching existing test
module instead of running broad eval as the primary proof.

## Non-required Work

- No local LLM eval is required for pure exclusion decisions.
- No broad sign-off is required for docs-only coverage decision updates.
- No focused fixture should be added just to prove that an excluded Anvil UI or
  advisory quality surface remains excluded.

## Review Notes

- This worklist intentionally keeps focused proof conditional so Phase30 does
  not manufacture model-facing behavior for rows that are excluded by design.
- If adoption is chosen, this file becomes the source of the exact test or
  fixture obligation before implementation.
