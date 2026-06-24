# Phase30 Row Closure Matrix

Date: 2026-06-23 JST

Status: completed / closed_excluded

| row | owner layer | current state before Phase30 | planned disposition | proof artifact or command | closure condition |
| --- | --- | --- | --- | --- | --- |
| C49 | Eval/recovery taxonomy decision | `Missing / Missing` in coverage table. | `excluded_with_rationale` | Coverage update, Phase30 decision report, `git diff --check`, `python3 tests/test_eval_report.py`. | Closed: semantic quality confirmation and advisory feedback classification are excluded; deterministic eval/report taxonomy remains. |
| C50 | CLI/REPL/slash command decision | `Partial / Missing` in coverage table. | `excluded_with_rationale` | Coverage update, Phase30 decision report, `git diff --check`, `cargo test slash_command --lib`. | Closed: Anvil UI helper compatibility is excluded; CommandAgent-native slash parser remains. |

## Disposition Rules

| disposition | allowed in Phase30 | required evidence |
| --- | --- | --- |
| `excluded_with_rationale` | Yes. Preferred for semantic quality confirmation and Anvil-specific UI helpers. | Source alignment, CommandAgent design rationale, and coverage row update. |
| `closed_proven` | Yes, but only for small deterministic behavior. | Runtime/doc/eval change plus targeted test or focused fixture. |
| `split_forward` | Yes, only before Phase32. This is the recorded form for partial adoption. | Failed proof evidence, narrower same-surface blocker, downstream phase, owner, and closure condition. |
| `blocked_external` | No. | C49/C50 are decision rows, not provider/network/environment blockers. |

## Review Notes

- C49 and C50 cannot remain `Missing` after Phase30.
- Broad sign-off is supplementary regression evidence and cannot be the sole
  closure proof.
- A row can be excluded only if the rationale explains why the omitted Anvil
  behavior is outside CommandAgent's intended architecture.
