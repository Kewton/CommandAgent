---
name: commandagent-release-post
description: Draft copy-ready release announcement text for a CommandAgent release. Use when the user asks for a release post, social post, announcement, or changelog-based summary.
---

# CommandAgent Release Post

Draft announcement text only. Do not post to any network service.

## Inputs

- release tag or version
- previous tag when available
- relevant changelog entries
- notable user-facing changes

## Evidence Commands

Use read-only commands:

```bash
git tag --list
git log <from>..<to> --oneline
```

Read `CHANGELOG.md` if present. If there is no changelog entry, say so and base the draft on commit messages only.

## Output

Provide:

- concise title
- short release summary
- bullet list of notable changes
- install or verification hint only if documented in `docs/release.md`
- link placeholder for the GitHub Release

Avoid unsupported claims such as fixed MSRV, binary signing, Homebrew distribution, or SBOM unless the repo docs confirm them.
