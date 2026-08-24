- Status: `passed`

## Local checks

- `cargo test --test release_distribution`: `passed` (5 tests)
- `shellcheck scripts/install.sh`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed` (1,531 library tests passed, 15 ignored; all integration and doc tests passed)
- `cargo publish --dry-run --locked`: `passed` (218 files packaged and verified; upload aborted as required by dry-run)

## GitHub prerelease UAT

- Final tag: `v0.1.0-rc.20260720.2`
- Commit: `00ba0fe37eee2e5af0fed5a6e14def32f5265f4c`
- Workflow run: `29734342725` (`passed`)
- Release: <https://github.com/Kewton/CommandAgent/releases/tag/v0.1.0-rc.20260720.2>
- Release state: published prerelease (`isDraft=false`, `isPrerelease=true`)
- Release assets: `passed` (four target archives and four matching SHA-256 files)
- Targets: `aarch64-apple-darwin`, `x86_64-apple-darwin`,
  `x86_64-unknown-linux-gnu`, and `x86_64-unknown-linux-musl`
- Installer UAT: `passed`; `scripts/install.sh` downloaded the Apple Silicon
  archive, verified SHA-256, and installed the binary.
- Installed binary version: `commandagent 0.1.0 00ba0fe 2026-07-20T10:19:20Z`
- Current Actions majors (`checkout@v7`, `upload-artifact@v7`,
  `download-artifact@v8`, `action-gh-release@v3`) completed without the
  Node.js 20 deprecation annotation.

## Retained diagnostic evidence

- `v0.1.0-rc.20260720` / run `29732897843` exposed the original macOS
  checksum and retired-runner defects and remains unchanged as evidence.
- `v0.1.0-rc.20260720.1` / run `29733766077` passed after those fixes and
  exposed the obsolete Node.js action-runtime warning.
- `v0.1.0-rc.20260720.2` is the final passing UAT run after all fixes.
