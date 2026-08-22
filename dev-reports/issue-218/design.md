# Issue #218 design

## Scope and predecessor review

- The assigned CLI reproductions are `--pack-verify /nonexistent`,
  `--extension-root /nonexistent --doctor`, and
  `--run-plan /nonexistent.yaml`.
- Required predecessor commit `e5b0bbca` was inspected. It changes CLI/config
  behavior for presets, runs, tracing, and runtime paths, but does not change
  the pack I/O or plan-path error leaves used by these reproductions.
- Keep `src/config.rs` and `src/cli.rs` untouched. Preserve exit codes, doctor
  checks, pack conformance, path confinement, and all underlying OS details.

## Design

1. Keep each leaf error self-contained with its path and OS cause, but do not
   also expose that already-rendered OS cause as a second error-chain entry.
   Apply this only to pack I/O and extension-manifest I/O errors that currently
   render the source inline.
2. Add the requested absolute plan path as context when canonicalization fails.
   Also attach the resolved path to the subsequent plan read so a filesystem
   race remains diagnosable.
3. Add focused process-level tests that require each command to retain its path
   and OS cause while emitting the OS cause only once.

## Verification plan

- Run the focused Issue #218/#220 CLI integration test first.
- Run formatting, Clippy for all targets, and the full Rust test suite because
  shared CLI error leaves are touched.
