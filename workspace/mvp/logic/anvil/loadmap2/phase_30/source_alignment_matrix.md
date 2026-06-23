# Phase30 Source Alignment Matrix

Date: 2026-06-23 JST

Status: completed / closed_excluded

## Matrix

| row | Anvil source files | Anvil responsibility | CommandAgent current surface | planned adopted behavior | planned omitted behavior | target module or doc | proof method |
| --- | --- | --- | --- | --- | --- | --- | --- |
| C49 | `quality.rs`, `quality_confirm.rs`, `feedback_kind_confirm.rs`, `task_classification.rs` | Classify task/feedback/quality and confirm quality-related decisions through advisory logic. | Eval/report categories, verifier/profile/setup/tool failure kinds, implementation-quality limitations, deterministic recovery evidence. | None in runtime. Keep existing deterministic eval/report taxonomy. | Model-powered quality confirmation, semantic app scoring, advisory feedback loop, provider/model-specific quality policy. | `docs/eval/legacy-control-stack-coverage-20260621.md`; Phase30 report. | `excluded_with_rationale`; `git diff --check`; `python3 tests/test_eval_report.py`. |
| C50 | `slash_commands.rs`, `plan_sections.rs`, `plan_mode_helpers.rs`, `commands.rs`, `tool_display.rs`, `message_push.rs`, `footer.rs` | Provide slash/plan command UI helpers, rendering helpers, footer/progress/message display, and command affordances. | CommandAgent-native CLI, REPL, slash command parser, `/plan-run`, `/ultra-plan-run`, docs, and tests. | None from Anvil. Keep CommandAgent-native slash command implementation. | Wholesale slash-command import, plan-mode UI compatibility, spinner/footer/rendering parity, Anvil-specific command affordances. | `docs/eval/legacy-control-stack-coverage-20260621.md`; Phase30 report. | `excluded_with_rationale`; `git diff --check`; `cargo test slash_command --lib`. |

## Review Notes

- The source mapping intentionally separates source file existence from
  adoption. Anvil having a helper does not make the helper a CommandAgent
  requirement.
- Both rows default to exclusion because their Anvil responsibilities are
  advisory/UI surfaces unless deterministic evidence proves otherwise.
- If adoption occurs, the implementation target must be CommandAgent-native
  and narrower than the full Anvil source family.
