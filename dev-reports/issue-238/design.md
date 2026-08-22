# Issue #238 Design

## Scope and baseline

Implement the approved tool-boundary change in `src/tools/registry.rs` and a
new leaf module under `src/tools/`. Do not add behavior to
`src/minimal_loop/loop_run.rs`. The worktree has no required predecessors. It
already contains Issue #207's `PostWriteCompletionTracker`, which honestly
completes a contract-free direct minimal-loop run after a successful write and
two consecutive successful read-only confirmation batches.

The branch is one orchestration-only commit behind `origin/develop`; that
commit changes only `scripts/codex_orchestrate.py` and its Python tests, so it
does not affect this Issue's code or contracts.

## Repeated-read state

Add a per-`ToolRegistry` repeated-read cache. A cache key consists of the
resolved file path and the optional `start_line` / `end_line` range, so only a
semantically identical `Read` can reuse an earlier result. Cache entries keep
a SHA-256 fingerprint of the complete file plus the first 20 returned content
lines. Directories are not cached.

Before a repeated read, fingerprint the file and compare it with the cached
entry:

- when unchanged, return the workspace-relative path, a compact unchanged
  marker, and at most the cached first 20 content lines;
- when changed, execute the existing `read::run` path and return its full
  result, then refresh the cache entry;
- when fingerprinting is unavailable, fall back to the existing full read so
  cache bookkeeping cannot weaken read errors or path-policy enforcement.

Track whether the repeated request is consecutive with the previous executed
tool call and whether its path was successfully written by this registry.
Expose those facts in an additive `tool_read_unchanged` eval event and in the
compact guidance. Failed writes do not become completion candidates.

`ToolRegistry` clones share the same cache through synchronized state, matching
the registry's existing immutable execution API while keeping a new registry
session isolated.

## Completion and compatibility

Do not duplicate or weaken Issue #207's completion gate. In the target
sequence, the first post-write `Read` is full, the identical second `Read` is
compact, and Issue #207's second confirmation-batch observation completes the
run on that same turn. Reads without a successful write remain unable to infer
completion, and completion-contract, required-path, plan-step, failed-command,
and failed-mutation interlocks remain unchanged.

No existing event is renamed or reshaped. The new event is additive, and tool
result strings have a compact marker only for repeated unchanged file reads.

## Tests and corpus

Add focused leaf tests for unchanged compaction, the 20-line bound,
non-consecutive calls, and changed fingerprints. Add registry tests proving an
unchanged identical read is compact, an intervening file change restores the
full response, and a failed write cannot mark a read as a completion
candidate.

Add an Issue #238 corpus golden that records the write, first full read, second
compact identical read, and existing post-write completion stop within four
turns (below the eight-turn acceptance bound). Run focused tool tests and the
corpus regression first, then formatting, Clippy, and the full Rust suite
because `ToolRegistry` is shared behavior.
