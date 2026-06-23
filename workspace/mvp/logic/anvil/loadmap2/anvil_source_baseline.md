# Anvil Source Baseline

Date: 2026-06-23 JST

## Baseline

| Field | Value |
| --- | --- |
| Repository | `/Users/maenokota/share/work/github_kewton/Anvil-develop` |
| HEAD | `b3ca3d330546a10bf90d8dd46bd3e102f1710573` |
| Dirty state | dirty |
| Coverage table | `docs/eval/legacy-control-stack-coverage-20260621.md` |

This baseline fixes the source inventory used by the loadmap2 migration
roadmap. The coverage rows C01-C54 are keyed to this Anvil HEAD. Dirty and
untracked Anvil files below are recorded for reproducibility, but they are not
automatically part of the migration surface.

## Dirty Tracked Files

`git diff --stat` at inventory clarification time:

```text
scripts/codex_orchestrate.py    | 2 +-
tests/test_codex_orchestrate.py | 4 ++--
2 files changed, 3 insertions(+), 3 deletions(-)
```

Treatment:

| path | treatment | rationale |
| --- | --- | --- |
| `scripts/codex_orchestrate.py` | excluded from current C01-C54 parity baseline unless a later baseline refresh adopts it | The loadmap2 migration targets recovery-control responsibilities, not dirty local orchestration edits. |
| `tests/test_codex_orchestrate.py` | excluded from current C01-C54 parity baseline unless a later baseline refresh adopts it | Test deltas are not used as source responsibility definitions in the current coverage table. |

## Untracked Files

Observed untracked paths:

```text
.agents/skills/source-command-acceptance-test/
.agents/skills/source-command-apply-review/
.agents/skills/source-command-architecture-review/
.agents/skills/source-command-create-pr/
.agents/skills/source-command-current-situation/
.agents/skills/source-command-design-policy/
.agents/skills/source-command-issue-create/
.agents/skills/source-command-issue-enhance/
.agents/skills/source-command-issue-split/
.agents/skills/source-command-issues-exec-plan/
.agents/skills/source-command-multi-stage-design-review/
.agents/skills/source-command-multi-stage-issue-review/
.agents/skills/source-command-pm-auto-design2dev/
.agents/skills/source-command-pm-auto-dev/
.agents/skills/source-command-pm-auto-issue2dev/
.agents/skills/source-command-pr-merge-pipeline/
.agents/skills/source-command-progress-report/
.agents/skills/source-command-refactoring/
.agents/skills/source-command-tdd-impl/
.agents/skills/source-command-uat-fix-loop/
.agents/skills/source-command-work-plan/
.claude/commands/release.md
.codex/prompts/uat-manual-check.md
.codex/skills/
scripts/photon_weekly_observe.sh
```

Treatment:

| path group | treatment | rationale |
| --- | --- | --- |
| `.agents/skills/source-command-*` | excluded from current C01-C54 parity baseline | Command/skill harness migration is outside this Anvil recovery-control stack closure unless a later phase opens a separate harness surface. |
| `.claude/commands/release.md` | excluded from current C01-C54 parity baseline | CLI/command UX helpers are tracked by C50 and default to exclusion unless recovery/eval evidence requires adoption. |
| `.codex/prompts/uat-manual-check.md` and `.codex/skills/` | excluded from current C01-C54 parity baseline | Prompt/skill surfaces are not source recovery-control responsibilities for the current migration table. |
| `scripts/photon_weekly_observe.sh` | excluded from current C01-C54 parity baseline | Photon/PAM advisory surfaces are explicitly excluded by the roadmap. |

## Refresh Rule

If a later review decides that any dirty or untracked Anvil file above should
affect CommandAgent parity, first refresh the baseline:

1. record a new Anvil HEAD and dirty state;
2. update the coverage table and Cxx mappings;
3. update `current_issue_phase_map.md`;
4. only then open implementation phases against the new responsibility rows.
