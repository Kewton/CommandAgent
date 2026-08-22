# Issue #230 implementation summary

## Outcome

Implemented the approved CLI-owned safety policy without changing GUI delegate
code, event schemas, corpus contracts, or the live `.anvil/` namespace.

## Changes

- Added `--allow` selectors for `read`, `write`, and `bash:verify`. An explicit
  list is enforced as a hard tool-class ceiling before execution; selected
  mutating classes are auto-approved. `--yes` remains the all-tools alias and
  does not bypass existing Bash confinement or dangerous-command checks.
- Centralized authorization in `src/tools/allow_policy.rs` and applied the same
  boundary to ordinary registry dispatch and the split-Bash execution path.
  `bash:verify` reuses the shared verifier command normalization and rejects
  recognized direct filesystem mutation.
- Added bounded, read-only Git inspection. Runs warn about pre-existing dirty
  state or an unmanaged/uninspectable workspace, then report the final tracked
  diff stat and untracked files on stderr at exit.
- Defined the exact `--offline` boundary once in a tools leaf and reused it in
  CLI help and the structured doctor check. The output explicitly states that
  provider/API requests and other network-capable commands are unaffected.
- Updated English and Japanese CLI references, security documentation, the
  flag count, and the changelog.

## Tests

- Added focused policy tests proving omitted Bash authority is denied before a
  command can execute, while allowed verifier commands retain approval and
  safety checks.
- Added Git-state unit and CLI integration coverage for startup warnings and
  exit reporting.
- Added CLI parsing/help and doctor JSON coverage for the new public contract.
- Ran the focused checks plus formatting, Clippy, documentation drift checks,
  and the complete Rust test suite. Exact results are recorded in
  `verification.md`.
