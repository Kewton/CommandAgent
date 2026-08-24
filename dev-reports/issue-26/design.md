# Issue 26 design

Add a plain GitHub Actions release workflow that tests and builds locked release
archives for the supported macOS and Linux targets, then uploads checksums and
generated release notes. Keep installation self-contained in `scripts/install.sh`
and document binary installation, source setup, crates.io dry-run metadata, and
a future Homebrew tap without creating external state.

The existing installer and release-distribution tests already define the archive
and checksum contract, so production code changes are limited to CI and docs.
