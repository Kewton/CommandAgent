# Issue #31 Design

## Scope

Add one repository-owned release-build command and focused tests for its
publishing boundary. Keep `cargo build`, `cargo test`, Cargo profiles, build
metadata generation, and the `target/release/commandagent` launcher path
unchanged. No runtime event, recovery, schema, or corpus contract changes are
needed.

## Design

- Add `scripts/build-release.sh`. It builds the optimized `commandagent` binary
  with `--locked` in a uniquely named temporary Cargo target directory under
  `target/`, leaving the ordinary Cargo target and cache behavior untouched.
- Run the staged binary's `--version` before publishing it. Require the package
  version plus the current short Git commit and dirty suffix produced by the
  existing `build.rs`; a build that cannot prove that provenance fails before
  modifying the published executable.
- Publish only after build and verification succeed. Preserve the existing
  `target/release/commandagent` until the final same-filesystem rename, then
  remove every other entry from `target/release` so stale hashed libraries,
  link-time artifacts, and metadata do not remain in the clean release output.
- Install an exit trap immediately after creating the temporary build
  directory. It removes build and publish staging directories on success and
  on every failure path.
- Document this script as the release/local-symlink build command. The
  resulting executable remains at `target/release/commandagent`, so an
  existing `commandagentdev` symlink to that path remains compatible.

## Failure Semantics

Compilation and provenance verification happen entirely in staging. Either
failure exits nonzero, deletes staging, and leaves a previously published
executable byte-for-byte unchanged. Cleanup of old release artifacts occurs
only after the candidate has passed verification, and the old executable is
excluded from that cleanup until the candidate is atomically renamed over it.

## Focused Tests

Add a Unix integration test that runs the real release script in isolated Git
fixtures with a fake Cargo executable. Cover successful publication and stale
artifact cleanup, a failed build preserving the old executable, a candidate
with incorrect version provenance preserving the old executable, staging
cleanup on all paths, and a `commandagentdev` symlink executing the published
path.

## Overfitting Review

This change fossilizes only the repository's published executable location and
the existing `--version` provenance shape. It narrows release-maintainer
freedom to publish unverified binaries or keep Cargo intermediates beside the
executable, but it does not constrain normal Cargo workflows or packaging that
copies the executable elsewhere. If the version schema or artifact layout must
change later, the honest degradation path is for this command to fail before
publication until its verifier and focused compatibility tests are updated,
rather than accepting ambiguous provenance or weakening cleanup.

## Verification

Run the focused release-build integration test first. Then run formatting,
all-target Clippy, the full Rust test suite, the real clean release command,
the published binary's `--version`, a `target/release` contents check, and
`commandagentdev --version` because this is release-sensitive shared tooling.
