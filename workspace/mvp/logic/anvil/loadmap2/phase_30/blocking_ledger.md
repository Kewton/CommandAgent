# Phase30 Blocking Ledger

Date: 2026-06-23 JST

Status: completed / closed_excluded

| blocker | row | incomplete contract | suspected module or doc | required decision | proof command or artifact | closure condition |
| --- | --- | --- | --- | --- | --- | --- |
| P30-C49-001 | C49 | Quality classification/confirmation was unresolved as `Missing`. | `docs/eval/legacy-control-stack-coverage-20260621.md`, eval taxonomy, recovery evidence docs. | Exclude semantic/advisory quality confirmation. | Phase30 C49 decision record and coverage update. | Closed as `excluded_with_rationale`. |
| P30-C49-002 | C49 | If adoption were claimed, the deterministic trigger and owner layer would need to be named. | `scripts/eval_report.py`, recovery evidence producers, verifier/profile/setup failure taxonomy. | No adoption. Existing deterministic taxonomy is sufficient; semantic quality scorer remains out of scope. | `python3 tests/test_eval_report.py`. | Closed; no semantic quality scorer or model confirmation loop added. |
| P30-C50-001 | C50 | Slash/plan/command UI helper parity was unresolved as `Partial / Missing`. | `src/agent/slash_command.rs`, `src/agent/repl.rs`, `docs/ultra-plan-run.md`, coverage docs. | Exclude Anvil UI helper compatibility. | Phase30 C50 decision record and coverage update. | Closed as `excluded_with_rationale`. |
| P30-C50-002 | C50 | If adoption were claimed, the CommandAgent-native UX/eval gap would need to be named. | CLI/REPL/slash parser tests and docs. | No adoption. Existing CommandAgent-native slash parser remains the supported surface. | `cargo test slash_command --lib`. | Closed; no wholesale Anvil slash command, footer, spinner, or rendering helper import. |

## Review Notes

- These blockers are decision blockers, not implementation blockers by
  default.
- `blocked_external` is not an acceptable outcome for these rows because the
  missing work is not provider, network, or environment constrained.
- If a new failure is discovered during Phase30, it must be split only when it
  is narrower than C49/C50 and has failed proof evidence.
