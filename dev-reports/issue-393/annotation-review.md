# Goal-to-verify v0 annotation review

- Corpus: `eval/goal_verify/v0/corpus.json`
- Review date: 2026-08-25
- Label author role: `issue-393-corpus-author`
- Blind reviewer role: `issue-393-blind-review-pass-1`
- Outcome: accepted

The reviewer checked claim IDs against deterministic observations without the
author's proposed verdicts visible, then compared the resulting allowed and
forbidden verdict partitions. All create/fix/investigate positive and negative
families and all required adversarial tags are represented.

One disagreement was recorded for `investigate-composite-timeout`: the review
initially required `unverified`, while the author allowed `partial`. Resolution:
v0 never coerces composite intent to create; `partial` is allowed only for the
two individually bound boundary observations, while `full` remains forbidden.
This resolution is frozen in the ADR and corpus metadata.
