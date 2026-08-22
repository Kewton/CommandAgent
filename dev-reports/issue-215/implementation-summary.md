# Issue #215 Implementation Summary

## Outcome

Headless direct `--prompt` runs without `--yes` now warn at startup that they
cannot approve mutating tools. When a mutating tool is attempted, the approval
error advertises only the executable `--yes` rerun and qualifies it for trusted
workspaces; it no longer suggests unavailable interactive approval.

## Changes

- Added `src/tools/approval.rs` as the single owner of the headless startup
  warning and approval-denial wording.
- Added minimal startup wiring in `src/lib.rs`, scoped to `Action::Prompt` with
  non-TTY stdin and no `--yes`.
- Routed the existing mutating-tool guard in `src/tools/registry.rs` through
  the new approval helper without changing which tools require approval or
  weakening the gate.
- Added leaf tests for warning eligibility, denial wording, automatic
  approval, and interactive approval.
- Added process-level headless tests proving the warning precedes provider
  startup failure and is absent when `--yes` is supplied.

## Compatibility

No event names or schemas changed. TTY prompt runs, `--yes` prompt runs, REPL,
planning actions, and unrelated CLI presentation retain their existing output.
The registry edit is limited to the approval guard and remains compatible with
Issue #238's committed repeated-read changes later in the same execution path.
