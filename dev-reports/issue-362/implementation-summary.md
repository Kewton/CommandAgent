# Issue #362 implementation summary

## Outcome

GUI Trial `plan-run` and `ultra-plan-run` now generate their automatic
completion contracts inside the isolated session execution workspace. A
Next.js `core-implementation` step can therefore load the generated contract
without weakening the existing workspace-or-temp safety boundary.

## Changes

- Added a small planner leaf module that resolves the generated contract
  location. It preserves the existing event-adjacent location when that
  directory canonically belongs to the execution workspace; otherwise it uses
  the workspace-owned `.commandagent/` directory.
- Kept explicit completion-contract normalization unchanged. Contracts outside
  the execution workspace and allowed system temp roots remain rejected, and a
  symlinked event directory cannot redirect automatic generation outside the
  workspace.
- Updated the focused Next.js `plan-run` and `ultra-plan-run` tests to model the
  GUI split between `sessions/<session-id>/` and the central
  `.commandagent/runs/<session-id>/` record. Both generated contract names and
  their workspace-relative event paths are fixed by assertions.
- Extended the GUI delegation regression to prove that the initial run and the
  Gate 3/4 continuation both pass the same canonical session workspace through
  `--cwd`, and wait for the continuation lease to return to `idle` before
  exercising legacy session lookup.
- Removed one byte-identical duplicate GUI test introduced by prior merged
  history. The remaining copy keeps the same Ollama `think` coverage and lets
  the GUI feature suite compile with one unique test name.
- Documented the completion-contract location alongside the existing GUI
  session workspace and central run-record boundaries.

## Compatibility and scope

The GUI events, state, summary, artifacts, session index, and lease storage
paths are unchanged. No event name or schema changed, no historical evidence
was rewritten, and the live `.anvil/` namespace was not modified. The
`src/planner/runner.rs` and `src/minimal_loop/loop_run.rs` growth tripwires were
untouched, and no guardrail baseline was raised.
