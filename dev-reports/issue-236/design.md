# Issue #236 Design

## Problem

Planner verification currently records most non-timeout command failures as
artifact failures. Deterministic execution-environment failures such as exit
127, an unavailable script interpreter, or an executable permission error can
therefore be classified as implementation repair targets and consume bounded
model repair calls even though source edits cannot make the verifier run.

## Design

- Wire deterministic verifier execution-environment classification into
  `src/planner/verify.rs` before ordinary artifact and tool-usage handling,
  with the classifier isolated in the `verify` leaf module to respect the
  file's growth guard. Treat exit 127, exit 126, missing-interpreter
  diagnostics, and execution-permission diagnostics as verifier-command false
  negatives.
- Reuse the existing non-repairable `VerifierCommand` reachability path so a
  plan-run stops without invoking the repair model. Do not change event names,
  schemas, runner control flow, or repair-loop limits.
- Keep ordinary nonzero test failures and Python syntax failures as artifact
  failures so their existing implementation repair behavior is unchanged.
- Preserve Issue #204's `python3 -m compileall -q src` timeout substitution and
  all existing Python interpreter selection behavior.
- Add focused classification tests plus a plan-run regression proving an exit
  127 verify failure makes zero repair-model calls.

## Verification

Run the focused verifier and plan-run regression tests first, then formatting,
clippy, and the full Rust test suite because shared planner verification
classification is affected.
