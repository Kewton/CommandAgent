# Issue #31 Implementation Summary

## Implemented

- Added executable repository command `scripts/build-release.sh`.
- Built the optimized `commandagent` binary with
  `cargo build --release --locked --bin commandagent` in a uniquely named
  temporary Cargo target.
- Verified the staged binary's `--version` output against the root package
  version, current short Git commit, dirty suffix, and a non-empty build
  timestamp before publication.
- Kept any existing `target/release/commandagent` in place throughout build and
  verification, then replaced it with a same-filesystem rename only after the
  candidate passed.
- Removed all non-executable entries from `target/release` after successful
  verification and installed exit traps that remove build and publish staging
  directories on success, build failure, verification failure, and signals.
- Updated the README to make the clean release script the documented command,
  describe failure preservation and cleanup, and retain the established
  `target/release/commandagent` path used by launcher symlinks.

## Tests

Added `tests/release_build.rs` with isolated Git fixtures and a fake Cargo
executable. Its three focused tests cover:

- successful optimized-candidate publication, stale `deps`/build artifact
  removal, staging cleanup, and `commandagentdev` symlink execution;
- induced Cargo build failure preserving the previous executable and removing
  temporary artifacts; and
- induced provenance-verification failure preserving the previous executable
  and removing temporary artifacts.

No production Rust, event, recovery, schema, `.anvil/`, corpus, Cargo profile,
or ordinary Cargo cache contract changed.
