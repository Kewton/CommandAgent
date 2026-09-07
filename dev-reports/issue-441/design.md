# Issue #441 design

## Inputs and scope

Read the complete saved Issue #441 (including its empty comments), approved
action-items A-3, parallel-development plan, worker skill, and development
guardrails before implementation. The worktree starts at `36f73eeb`; there are
no required predecessors. Existing #428 path-token tests are present.
The referenced develop worktree and September 7 campaign remain read-only.

## Change

- Detect literal `<user>` and `<redacted>` before Bash command normalization
  and redirect parsing, using `tools/placeholder_path.rs`. Reject without
  guessing the actual username or executing a repaired command. Give explicit
  workspace-relative retry guidance in both direct tools and runtime policy.
- Preserve existing event names, schema version, and confinement error kind;
  add a placeholder discriminator and SHA-256 of the exact command string.
  Store its first 64 raw bytes and hash only in owner-private evidence beneath
  `.commandagent/evidence/`. Reject unsafe evidence directories/symlinks and
  report recording failures without permitting execution.
- Normalize known workspace-root prefixes before recovery evidence redaction,
  including already anonymized versions of that exact root. Preserve component
  boundaries; do not turn sibling/outside/traversal paths into workspace paths.
  Suppress remaining anonymized external paths with a non-executable marker.
  Put this display-only logic in a repair leaf module and apply it to saved
  recovery prompts/reports and generated recovery plans. The typed automatic
  recovery caller and the saved YAML must use the same root-aware builder;
  retain the existing strict equality check and original handoff authority.
- Follow A-3 and the original Issue's placeholder repeat exemption: a leaf-state
  call excludes only exact Bash placeholder rejections from the generic error
  counter. This lets a batch of I2 calls return feedback before a model retry;
  ordinary confinement errors retain their counter and all step iteration/time
  caps remain. Do not edit `loop_run.rs` or weaken verification. Test a single
  diagnostic and the historical batch followed by a scripted correction; this
  does not claim that an unchanged model will always follow the feedback.

## Verification

Add an Issue-specific I2 events 196–213 fixture with explicit provenance, plus
executable regressions for detection, non-execution, private evidence/hash,
recovery path normalization, and one-error-then-corrected runtime continuation.
Run focused tests first, retain #428 and workspace confinement coverage, then
format, warning-free all-target clippy, full Rust tests, corpus regression,
generality guardrails, and conformance. No dependency, release, campaign,
GitHub, or live runtime-state changes are planned.
