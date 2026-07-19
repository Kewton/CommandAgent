---
name: orchestrate
description: Plan and run CommandAgent GitHub issue orchestration through dependency-aware bounded parallel worktrees, CommandMate Codex workers, verified draft pull requests, CI, evidence-backed UAT, and guarded dependency-order merging. Use when the user asks to orchestrate one or more issues in this repository.
---

# CommandAgent Orchestrate

Plan issue work first, then advance only through the phases the user has authorized.

## Operating Rules

- Start from the CommandAgent `develop` integration worktree.
- Use `origin/develop` as the base for planned worktrees.
- Keep issue enhancement lightweight and ask only blocking questions.
- Store run artifacts under `workspace/management/runs/<run_id>/`.
- Treat existing files under `workspace/management/runs/` as frozen historical evidence.
- Do not delete, reset, or overwrite existing worktrees without explicit approval.
- Do not create or merge pull requests unless the user has authorized those external actions.
- Create pull requests as drafts. Do not mark them ready until worker verification, CI, and UAT pass.
- Never merge with failing or unavailable CI, incomplete UAT evidence, or a blocking UAT result.
- Do not start or stop CommandMate, or kill port processes, unless the user explicitly asks.
- Treat `commandmatedev` localhost read failures as `unreachable` until verified outside the sandbox. A sandbox failure does not prove that the user's CommandMate server is stopped.
- Use the Codex CommandMate agent. The repository script defaults to `--agent codex` and waits on `--instance codex`.
- Treat `--max-parallel` as a hard positive concurrency limit. Never dispatch a batch wider than this value.
- Execute dependency batches in order. Dispatch a later batch only after every worker in the preceding batch completes and its committed verification report passes.
- Never place Issues with detected implementation-file overlap in the same batch. Reject cyclic dependencies, incomplete explicit merge orders, and explicit orders that place an Issue before its dependencies.
- When the user confirms dependencies that differ from inference, pass one `--dependency-override ISSUE:DEP,...` entry for every requested Issue. Use `ISSUE:` for a root Issue. Treat this complete graph as authoritative; never mix a partial override with inferred edges.
- Pass each resolved decision Issue as `--issue-decision ISSUE:TEXT`. State the exact approved scope and exclusions so the decision becomes an acceptance criterion and worker instruction.
- Reuse the exact dependency override and decision flags in every later phase of the same work. Never fall back to inference after an explicit plan is accepted.
- Include manual UAT steps when CLI/TTY behavior, release flow, GUI behavior, or a real device must be confirmed.

## Authorized Flow

Advance only through phases explicitly authorized by the user:

1. Plan: run and review the required dry-run.
2. Develop: create issue worktrees, then dispatch and verify Codex workers one bounded dependency batch at a time. Stop before dispatching later batches when any earlier worker fails dispatch, wait, or verification.
3. Verify: require each worktree's `dev-reports/issue-<number>/verification.md` to report `passed` with every recorded check passing. Stop on missing, failed, or ambiguous evidence.
4. Pull request: push the issue branch and create or reuse a draft PR only after verification passes.
5. CI and UAT: wait for all PR checks, then execute or collect every generated UAT scenario with evidence. Read [UAT result input](references/uat-results.md) before this phase.
6. Merge: only after all PRs pass CI and every UAT scenario passes with evidence, mark drafts ready, recheck CI and mergeability, then map Issue dependency order to PR numbers and merge in that enforced order.

`--phase uat` must not merge. `--phase merge` must require `--uat-results-json`; missing or incomplete evidence blocks merging. A phase authorization does not authorize CommandMate start/stop, PR creation, or merging unless the user explicitly included that action.

## First Action

Run the non-mutating planner before any other phase:

```bash
python3 scripts/codex_orchestrate.py <issue...> --dry-run
```

For an approved dependency graph or decision, include the explicit inputs in the dry-run. For example:

```bash
python3 scripts/codex_orchestrate.py 15 16 17 --dry-run --max-parallel 2 \
  --dependency-override 15: \
  --dependency-override 16:15 \
  --dependency-override 17:16 \
  --issue-decision "17:Adopt Option A; preserve internal identifiers and update only docs/mechanism-ledger.md."
```

Review:

- `workspace/management/runs/<run_id>/manifest.md`
- `workspace/management/runs/<run_id>/issue-analysis.md`
- `workspace/management/runs/<run_id>/dependency-plan.md`

Confirm that the issue scope, dependency source, approved decisions, dependency batches, configured maximum width, suspected files, and blocking questions are coherent. Reject a plan that omits or contradicts a user-approved dependency or decision. The planner defaults to the `codex` CommandMate agent; override `--codex-agent-name` only when the user requests another registered instance.

## Advancing The Run

Use only the flags required for the authorized phase. Worktree creation, CommandMate dispatch, pull-request creation, merging, and UAT-fix worktrees are mutating operations. Report the generated run directory and stop before any unapproved phase.

The dispatched worker prompt invokes `$codex-issue-worker`. Keep that skill available in each issue worktree by committing this repository harness before dispatch.

Treat worker completion as necessary but insufficient for publication. Inspect the worker verification gate before creating a draft PR. Treat CI success as necessary but insufficient for merging; UAT must also pass with complete evidence.

For multi-Issue development, inspect `scheduler-report.md` and `commandmate-wait-report.md`. A completed batch must show every worker completed and passed verification. Do not manually bypass a blocked batch by dispatching its dependents. A later worker prompt lists required dependency or file-conflict predecessor branches and worktrees to inspect before editing; live dispatch reaches that prompt only after those predecessors pass, while dry-run output is only a plan. It does not claim those branches are already merged.

## Verification

After changing the harness, run:

```bash
ruff check scripts/codex_orchestrate.py tests/test_codex_orchestrate.py
pytest -q tests/test_codex_orchestrate.py
```
