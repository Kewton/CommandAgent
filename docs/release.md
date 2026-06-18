# Release Operations

CommandAgent releases are created from `v*` tags pushed to GitHub. The release
workflow builds platform binaries, packages them as gzip assets, writes SHA-256
checksum files, and creates a GitHub Release.

## Release Flow

The normal promotion path is:

```text
feature branch -> develop -> main -> v* tag -> GitHub Release
```

Feature work is merged to `develop` first. Release candidates are promoted from
`develop` to `main`. Tags are created only from commits that are already on
`main`.

## Pre-Release Checklist

- `main` contains the intended release commit.
- Required CI checks are passing on `main`.
- `Cargo.toml` package version matches the intended tag without the `v` prefix.
- Any release-blocking docs or known limitations are updated.
- Manual `Live E2E` validation has been run when the release needs live
  provider evidence.
- MSRV remains unresolved until Issue #1 is completed; do not claim a fixed
  minimum Rust version in release notes yet.

The `Live E2E` workflow is manual-only and is not a merge gate. Use it before a
release when Ollama, OpenAI, Gemini, or large eval evidence is needed.

## Create A Release

From the `main` worktree:

```bash
git fetch origin
git pull --ff-only
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

Pushing the tag triggers `.github/workflows/release.yml`.

The workflow preflight checks that:

- the tag starts with `v`
- the tagged commit is contained in `origin/main`
- the tag version matches `Cargo.toml`

## Release Assets

The workflow publishes:

- `commandagent-linux-amd64.gz`
- `commandagent-linux-arm64.gz`
- `commandagent-darwin-amd64.gz`
- `commandagent-darwin-arm64.gz`
- one `.sha256` file for each gzip asset

## Post-Release Verification

After the workflow completes:

```bash
gh release view v0.1.0 --repo Kewton/CommandAgent
COMMANDAGENT_VERSION=v0.1.0 scripts/install.sh
commandagent --help
```

For manual asset checks:

```bash
shasum -a 256 -c commandagent-linux-amd64.gz.sha256
gzip -dc commandagent-linux-amd64.gz > commandagent
chmod +x commandagent
./commandagent --help
```

## Failure Handling

If the release workflow fails before creating the GitHub Release, fix the issue
on a new branch, merge through `develop` and `main`, then create a new tag.

If the GitHub Release was created with incorrect assets, delete the release and
tag only after confirming no users depend on them. Recreate the release from a
corrected commit and a new tag.

## Follow-Up Scope

The current MVP release process does not include binary signing, Homebrew
distribution, SBOM generation, or a fixed MSRV policy. Track MSRV completion in
Issue #1.
