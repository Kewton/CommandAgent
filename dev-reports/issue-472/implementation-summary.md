# Issue #472 implementation

Read now returns a typed, actionable failure for an ordinary missing target
whose workspace and policy boundaries can be established. The model can
continue inspection or create a required file within the existing permission
and verification contracts. Read never manufactures content or success.

- Added Read-specific failure proof with before/after root and ancestor checks,
  actual selected-path binding and protected-input checks.
- Added target-specific unresolved-miss counting; unrelated successes and
  alternate path spellings do not reset the limit.
- Preserved existing failure events and added the `read_path_missing` kind.
  Moved path guidance to a leaf to satisfy the unchanged growth guard.
- Added production-loop/Runner positive and rejection tests, filesystem/race
  controls and a corpus consumed by the tests. The deterministic race hook is
  test-only and does not enter release builds.

Codex2 reviewed the design and candidate in five rounds. Round 4 requested a
growth-guard fix and an actual-registry fallback regression; both were added
and round 5 found them resolved. Its full-suite condition was still pending at
review time; the parent subsequently observed the completed, successful final
full run. See verification.md for the exact local checks and limitations.

Original develop changes and historical campaign evidence were preserved.
Neither the outer Recovery admission list nor the attempt/acceptance gates
were changed. Absolute/salvaged and symlink missing requests retain prior
errors; new recoverability is conservative on non-Unix systems. This is not a
measurement of generated business UI, persistent application data, automatic
promotion or live R0 success.
