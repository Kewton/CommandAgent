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

The approved release UAT created and retained prerelease tags and GitHub
prereleases. The final evidence tag is `v0.1.0-rc.20260720.2`; its four archives,
four checksums, prerelease metadata, and installer path were verified. No crates.io
publish or external Homebrew repository was created.
