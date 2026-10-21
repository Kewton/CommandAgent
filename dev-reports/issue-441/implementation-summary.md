# Issue #441 implementation summary

Literal `<user>` and `<redacted>` commands now receive `placeholder path detected`
and workspace-relative retry guidance before normalization, redirect parsing, or
shell execution. The runtime preflight uses the same detector, including Verify
steps; placeholder rejection never becomes deterministic verifier evidence.

## Acceptance criteria

1. `src/tools/placeholder_path.rs` supplies the diagnostic used by Bash and the
   runtime policy. Tool feedback preserves this diagnosis instead of calling it
   an outside absolute path. Literal redirects containing placeholders remain
   blocked. No username reconstruction or automatic command execution occurs.
2. `src/planner/repair/recovery_paths.rs` normalizes actual/canonical workspace
   prefixes and their known anonymized forms before redaction. Component and
   traversal checks keep sibling/outside paths from becoming relative paths.
   Remaining anonymized paths become a non-executable omission marker. Saved
   recovery prompts, exhausted repair reports, and generated recovery plan text
   use this transformation; whitespace, indentation, and UTF-8 are preserved.
   The original typed handoff remains the recovery candidate's authority source.
   Automatic recovery and YAML saving share the root-aware builder so the
   existing strict candidate/YAML equality gate continues to pass correctly.
3. Existing #428 and Bash workspace-confinement tests remain unchanged. Actual
   workspace `cd` prefixes still strip, outside redirects/references and symlink
   escapes still reject. Exact Bash placeholder rejections are excluded from
   the generic repeated-error counter so a multi-call batch can reach the next
   model turn. Ordinary errors keep their count even across these rejections;
   step iteration and wall-clock caps remain unchanged.
4. Each placeholder rejection records SHA-256 of the exact command string and
   its first `min(64, length)` bytes in
   `.commandagent/evidence/bash-placeholders/<uuid>.json`. The raw prefix is a
   JSON byte array, preserving even a split UTF-8 codepoint without redaction.
   The directory is mode 0700 and new files are mode 0600. Descriptor-relative
   no-follow opens reject symlink ancestors and avoid overwriting files.
   Evidence recording failure is reported and never permits execution.
5. Existing `bash_path_confinement_rejected` and `runtime_bash_policy` event names,
   schema version, and fields remain. Placeholder rejections add
   `placeholder_detected: true` and `command_sha256`; raw prefix bytes remain
   private. The existing confinement error kind is retained, while operation /
   violation values distinguish placeholders from outside paths.

## Regression evidence and boundaries

- `tests/corpus/apps/issue441-placeholder/fixtures/i2-events-196-213.jsonl`
  projects the original I2 diagnostics with source line numbers. Its provenance
  records the selected original bytes' hash; historical evidence was not edited.
- `tests/issue441_placeholder.rs` exercises the three historical command
  summaries, corrected relative reads, non-execution, exact hashes, raw Unicode
  bytes, private permissions, hostile evidence paths, and saved recovery output.
  A scripted client drives the real implementation step through one diagnostic,
  a corrected relative Bash read, a successful Write, and step verification.
  The same test also covers all three rejected commands from the historical
  runtime-policy events in one batch, followed by one corrective model turn.
- Leaf-module unit tests cover normalization boundaries, indentation/secrets,
  Verify policy, and evidence-write failure.
- The approved A-3 scope is implemented without changing the shared loop runner,
  verification gates, dependencies, guardrail baselines, or live `.anvil` state.
  Model compliance and campaign outcome remain for the orchestrator's campaign;
  continuing placeholder mistakes remain bounded by the unchanged step budgets.
  No push, PR, CI/UAT, or lifecycle operation was performed.
