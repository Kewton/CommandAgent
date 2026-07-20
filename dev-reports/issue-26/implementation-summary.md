# Issue 26 implementation summary

- Added `.github/workflows/release.yml` for `v*` tags. It runs locked tests,
  builds GNU and musl Linux plus Intel/Apple Silicon macOS release binaries,
  packages versioned archives, verifies checksums, and creates generated-note
  GitHub Releases.
- Documented verified binary installation, source setup distinction, crates.io
  dry-run/publish considerations, and the future Homebrew tap proposal in both
  README languages.
- Kept the existing installer contract and made its PATH guidance ShellCheck
  clean.

No external tag, GitHub prerelease, or Homebrew repository was created from this
worker because those are external release-state mutations; the workflow is ready
for maintainers to perform the approved UAT action.
