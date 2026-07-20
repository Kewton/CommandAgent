---
name: orchestrate
description: Plan and run CommandAgent GitHub issue orchestration through dependency-aware bounded parallel worktrees, CommandMate Codex workers, verified draft pull requests, CI, evidence-backed UAT, and guarded dependency-order merging. By default, continue through pull-request creation and merge unless the user limits the scope. Use when the user asks to orchestrate one or more issues in this repository.
---

# CommandAgent Orchestrate

Plan issue work first, then continue through verified draft pull requests, CI, evidence-backed UAT, and guarded merging unless the user sets an earlier stopping point.

## Operating Rules

- Start from the CommandAgent `develop` integration worktree.
- Use `origin/develop` as the base for planned worktrees.
- Keep issue enhancement lightweight and ask only blocking questions.
- Store run artifacts under `workspace/management/runs/<run_id>/`.
- Treat existing files under `workspace/management/runs/` as frozen historical evidence.
- Do not delete, reset, or overwrite existing worktrees without explicit approval.
- Treat an invocation of `$orchestrate` with one or more Issues as authorization to create or reuse issue worktrees, dispatch CommandMate Codex workers, push issue branches, create or reuse draft pull requests targeting `develop`, run CI and UAT gates, mark passing drafts ready, and merge them in dependency order.
- Apply that standing authorization only to the invoked orchestration run. A user instruction such as `plan only`, `stop after development`, `do not create PRs`, or `do not merge` narrows the run and takes precedence.
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

## Default Guarded Flow

Unless the user narrows the scope, advance through every phase below without requesting another approval between phases:

1. Plan: run and review the required dry-run.
2. Develop: create issue worktrees, then dispatch and verify Codex workers one bounded dependency batch at a time. Stop before dispatching later batches when any earlier worker fails dispatch, wait, or verification.
3. Verify: require each worktree's `dev-reports/issue-<number>/verification.md` to report `passed` with every recorded check passing. Stop on missing, failed, or ambiguous evidence.
4. Pull request: push the issue branch and create or reuse a draft PR only after verification passes.
5. CI and UAT: wait for all PR checks, then execute or collect every generated UAT scenario with evidence. Read [UAT result input](references/uat-results.md) before this phase.
6. Merge: only after all PRs pass CI and every UAT scenario passes with evidence, mark drafts ready, recheck CI and mergeability, then map Issue dependency order to PR numbers and merge in that enforced order.

`--merge-method` defaults to `merge`. Always pass an explicit merge strategy to the non-interactive GitHub CLI; use `squash` or `rebase` only when the user requests it.

`--phase uat` must not merge. `--phase merge` must require `--uat-results-json`; missing or incomplete evidence blocks merging. The default authorization includes pull-request creation and merging, but it does not authorize starting or stopping CommandMate or killing processes. If CommandMate is unavailable, report that blocker and request the specific external action needed.

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

After a coherent plan passes review, continue with the default guarded flow. Pause only at a user-defined boundary, a blocking plan question, an unavailable required service, a failed verification or gate, incomplete UAT evidence, or another condition that would make proceeding dishonest or unsafe.

## Advancing The Run

Use only the flags required for the current phase and preserve the same explicit dependency and decision inputs throughout the run. Report the generated run directory as a progress update, then continue through the default guarded flow unless the user imposed a boundary or a gate blocks progress.

The dispatched worker prompt invokes `$codex-issue-worker`. Keep that skill available in each issue worktree by committing this repository harness before dispatch.

Treat worker completion as necessary but insufficient for publication. Inspect the worker verification gate before creating a draft PR. Treat CI success as necessary but insufficient for merging; UAT must also pass with complete evidence.

For multi-Issue development, inspect `scheduler-report.md` and `commandmate-wait-report.md`. A completed batch must show every worker completed and passed verification. Do not manually bypass a blocked batch by dispatching its dependents. A later worker prompt lists required dependency or file-conflict predecessor branches and worktrees to inspect before editing; live dispatch reaches that prompt only after those predecessors pass, while dry-run output is only a plan. It does not claim those branches are already merged.

## Verification

After changing the harness, run:

```bash
ruff check scripts/codex_orchestrate.py tests/test_codex_orchestrate.py
pytest -q tests/test_codex_orchestrate.py
```
