# Migration Progress

Updated: 2026-06-24

This file tracks completed migration tickets from the external migration work
plan. It is intentionally kept inside the CommandAgent repository so progress is
visible from commits, not only from local workspace notes.

| ID | Status | Evidence |
| --- | --- | --- |
| CA-0001 | Done | Rust CLI skeleton, `commandagent --help`, build/test passed. |
| CA-0002 | Done | Philosophy, architecture, usage, ADR, branding check. |
| CA-0003 | Done | `scripts/eval_smoke.sh` offline smoke. |
| CA-0101 | Done | Minimal config with CLI/env/config precedence tests. |
| CA-0102 | Done | Provider capability and planner target contract tests. |
| CA-0103 | Done | Path confinement, host validation, workspace path tests. |
| CA-0201 | Done | Read/Write/Edit tools and registry tests. |
| CA-0202 | Done | Glob/Grep hidden skip and limit tests. |
| CA-0203 | Done | Bash offline policy, mkdir trap, cd-wrapper tests. |
| CA-0301 | Done | Ollama mock transport and provider smoke script. |
| CA-0302 | Done | XML fallback parser and downgrade helper tests. |
| CA-0303 | Done | Gemini generateContent mock transport and smoke script. |
| CA-0304 | Done | OpenAI Responses API input/output content contract tests. |
| CA-0401 | Done | Session save/load/discovery/compact tests. |
| CA-0402 | Done | llm-io JSONL logging and secret masking tests. |
| CA-0501 | Done | Minimal loop one-shot Write, XML fallback, downgrade, final-answer contract tests. |
| CA-0502 | Done | Minimal loop future-action, completion-without-write, and requested-artifact feedback guard tests. |
| CA-0601 | Done | Minimal REPL line loop, prompt routing, per-turn session save, and usage docs. |
| CA-0602 | Done | Slash command parser, profile/style options, and safe repair-prompt file references. |
| CA-0701 | Done | Step plan schema, YAML render/load, generation/correction prompts, and plan saving. |
| CA-0702 | Done | Plan lint for expected path type, setup/verify separation, and verification-only regression. |
| CA-0703 | Done | Deterministic verifier, dependency_missing, diagnostic/source excerpts, and evaluation docs. |
| CA-0704 | Done | Repair budget, exhausted report, short repair packet, and suggested replan command. |
| CA-0801 | Done | Ultra plan schema, phase generation prompt, phase validation, and ultra-plan saving. |
| CA-0802 | Done | Ultra phase execution core, phase prompt construction, workspace snapshot, and failure-stop report. |
| CA-0803 | Done | MVP profile contracts, Next.js verifier hint, data raw-input protection, and profiles docs. |
| CA-0901 | Done | Eval case schema docs, three smoke cases, and six large-task skeleton cases. |
| CA-0902 | Done | Eval slice runner with run roots, meta.json, stdout/stderr, workspace, and summary.tsv. |
| CA-0903 | Done | Eval report generation and recheck_summary.tsv for existing run roots. |
| CA-0904 | Done | Large eval wrapper with runs=1 MVP default and runs=3 release-quality mode. |
| CA-1001 | Done | README quickstart, provider examples, eval commands, and known limitations link. |
| CA-1002 | Done | Philosophy, architecture, and minimal-only ADR clarify legacy removal, sidecar deferral, and responsibility boundaries. |
| CA-1003 | Done | Usage, ultra-plan-run, providers, profiles, and known-limitations docs clarify commands, options, and repair workflow. |

Latest verification:

```text
scripts/eval_smoke.sh
```

Result: passed.

Latest UAT:

```text
docs/eval/mvp-uat.md
```

Result: REPL slash-command dispatch is wired and covered by regression tests.
Live UAT now passes for docs, Python, Rust, a Next.js file-set workflow,
planner/executor split, and repair fallback prompt saving. Larger
`/ultra-plan-run` sign-off remains pending.

Latest Anvil migration sign-off:

```text
docs/eval/loadmap2-final-migration-decision-20260624.md
```

Result: `migration_complete_with_explicit_exclusions`.

Current local LLM proof roots are admitted with 3 smoke, 82 focused, and 6
large cases, for `91/91` current case coverage. Current broad sign-off exits
zero. Adopted Anvil control/recovery rows C01-C45 are implemented/proven, and
excluded rows C46-C54 remain explicit design exclusions.

The six current large rows are still failed application-generation tasks. They
are migration-safe only because each is classified as an owned failure with
action, target/evidence, and disposition; they are not counted as successful
generated applications.

Historical Phase32 evidence from 2026-06-23 remains regression evidence only.
It is superseded as the final migration proof because it covered a smaller
case set.

Latest MVP eval sign-off:

```text
docs/eval/mvp-eval-report.md
```

Result: offline smoke and live UAT pass. Clean `/plan-run` smoke eval is not yet
stable enough to be a release gate. The large eval runner supports per-case
`/ultra-plan-run` mode and fixture seeding, but large app-generation quality
remains a separate product/eval limitation from Anvil migration parity.

Latest large triage:

```text
docs/eval/triage/large-root-20260617T003924.md
```

Result: completed. The root shows migration parity concerns around generated
plan contracts, blocked read-only inspection commands, dependency policy for
large Next.js cases, and max-iteration observability. No performance tuning was
made as part of the triage.
