# Phase31 Focused Worklist

Date: 2026-06-23 JST

Status: completed / closed_proven

## Focused Proof Policy

Phase31 does not require focused model-facing fixtures by default. Its primary
proof is a fresh large eval root.

Focused or fixture proof is required only if Phase31 changes eval script
behavior, such as adding a no-timeout mode.

## Work Items

| item | row | trigger | proof |
| --- | --- | --- | --- |
| F31-LARGE-PROOF | P17-L001 | Always. | Fresh large root with recheck/sign-off. |
| F31-NO-TIMEOUT-DRY-RUN | P17-L001 | If no-timeout eval support is added. | Dry-run large eval proving CLI wiring without running model tasks. |
| F31-SIGNOFF | P17-L001 | Always after proof route. | `scripts/eval_signoff.py --require-recheck` over current smoke/focused roots and selected large root. |

## Completed Proof

- `F31-LARGE-PROOF`: `eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624`
- `F31-NO-TIMEOUT-DRY-RUN`: `/private/tmp/commandagent-phase31-large-dry-run/20260623T174543`
- `F31-SIGNOFF`: `status: pass`

## Candidate Commands

If eval scripts are unchanged:

```bash
git diff --check
python3 scripts/eval_report.py <large-root> --cases-dir eval/cases/large --recheck
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=<smoke-root> \
  --root focused=<focused-root> \
  --root focused-fixture=<fixture-root> \
  --root large=<large-root>
```

If no-timeout support is added:

```bash
git diff --check
python3 tests/test_eval_report.py
python3 tests/test_eval_signoff.py
scripts/eval_large_tasks.sh --dry-run --out /tmp/commandagent-phase31-large-dry-run --runs 1
```

## Non-required Work

- No focused runtime fixture is required just to prove large no-timeout eval
  wiring.
- No model-facing repair prompt changes are expected.
- No runtime or provider policy tests are expected unless implementation
  unexpectedly touches those layers.

## Review Notes

- The worklist names large proof explicitly because Phase31 is not a focused
  eval phase.
- If the large proof is too expensive to run in the current environment,
  Phase31 remains open rather than closing through an external limitation.
