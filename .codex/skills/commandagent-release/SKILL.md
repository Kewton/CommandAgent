---
name: commandagent-release
description: Prepare or verify a CommandAgent release using the repository release process. Use when the user asks for a release, version tag, release PR, or GitHub Release for CommandAgent.
---

# CommandAgent Release

Use `docs/release.md` as the source of truth. This skill is a checklist for applying that process.

## Safety Rules

- Release from `main`.
- Tags must point to commits already contained in `origin/main`.
- Do not tag before the release PR is merged to `main`.
- Do not force-push, force-tag, delete tags, or delete release branches unless the user explicitly approves.
- Keep `.env`, `.commandagent`, raw eval roots, and workspace artifacts out of release commits.

## Flow

1. Read `docs/release.md` and `.github/workflows/release.yml`.
2. Confirm `main` contains the intended release commit.
3. Confirm required CI checks are passing on `main`.
4. Confirm `Cargo.toml` version matches the intended tag without the `v` prefix.
5. Confirm release-blocking docs and known limitations are updated.
6. Tag from the `main` worktree:

```bash
git fetch origin
git pull --ff-only
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

7. Verify the GitHub Release and assets after the workflow completes.

Do not create a release if version, branch, tag, or CI evidence is unclear.
