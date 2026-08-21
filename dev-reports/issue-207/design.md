# Issue #207 Design

## Scope and predecessor audit

Implement the Epic #260 Lane B fix only for the direct minimal loop used by
`--prompt`. The required predecessor commits are already ancestors of this
worktree: Issue #236 is `3740d9fe` (deterministic verifier environment-failure
classification) and Issue #234 is `f848084e` (role-specific think settings and
the expanded `Config` initializer). Their committed design, implementation,
verification reports, and source diffs were inspected before this note.

Do not alter planner verification classification, shared `Config` resolution,
plan-run completion, completion-contract enforcement, or genuine no-progress
stagnation semantics.

## Completion rule

Add a leaf-module tracker for the otherwise unverified direct-prompt case. A
contract-free minimal-loop run may complete after post-write inspection only
when all of the following hold:

- this run successfully wrote or edited at least one workspace path;
- two consecutive successful tool batches contain only `Read` calls;
- every confirming `Read` targets a path successfully written in this run; and
- no failed Bash execution remains after the latest successful write or Bash
  execution.

A new write starts a fresh confirmation window. A non-Read batch resets the
consecutive confirmation count. A failed Bash blocks inferred completion so a
failed verification followed by inspection cannot become a false success; a
later successful Bash or write clears that blocker.

Reuse Issue #236's deterministic environment-failure classifier at the shared
Bash outcome boundary. If such a failure remains unresolved, preserve it as the
concrete exhaustion blocker instead of relabeling it as model no-progress.

When the bounded rule is satisfied, emit an additive `loop_stop` reason and
return the existing successful `AssistantFinal` outcome with text that states
the result is unverified. The existing direct-command finalizer will then
produce exit 0 / `completed` without claiming stronger acceptance evidence.

## Guardrails and tests

Keep `src/minimal_loop/loop_run.rs` changes to state initialization, outcome
observations, and one completion branch. Put the decision state and unit tests
in a new leaf module.

Add a model-independent regression that reproduces the Issue sequence: write
`hello.py`, edit `README.md`, successfully run `python3 hello.py`, then read
the two changed files in consecutive turns. Assert successful completion and
the absence of `model_stagnation:no_progress_recorded`. Preserve a focused
golden proving reads without any prior write still fail as genuine no progress.
Add corpus coverage for the new stop reason and document the bounded headless
`--prompt` behavior.

Run focused leaf and loop regressions first, then the corpus regression,
formatting, Clippy, and the full Rust suite because shared minimal-loop behavior
is touched.
