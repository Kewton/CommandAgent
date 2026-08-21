# Issue 245 design: co-located extension profiles and packs

## Observed defect

`--packs` supports both the legacy `<extension-root>/<id>/<version>` pack
layout and the documented `<extension-root>/packs/<id>/<version>` layout. The
legacy traversal currently interprets every first-level directory other than
`packs/` as a pack ID. In a documented extension root containing
`profiles/static-site/manifest.toml`, it therefore passes
`profiles/static-site/` to pack conformance and exits with an error before
listing the valid local pack.

## Change

- Keep both existing pack layouts and their ordering/deduplication behavior.
- Treat the documented top-level `profiles/` namespace as extension metadata,
  not as a legacy pack ID, when traversing the extension root.
- Do not loosen pack conformance: any candidate below a real pack layout still
  fails honestly if it is malformed.
- Record the user-visible fix in the changelog without changing CLI output or
  pack/profile schemas.

## Regression coverage

Add a CLI integration test whose extension root contains both
`profiles/static-site/manifest.toml` and a pinned
`packs/my-cli-pack/1.0.0`. The test will prove that:

1. `--packs` succeeds and lists `my-cli-pack@1.0.0` as local;
2. `--profile static-site` loads successfully from that root; and
3. `--pack my-cli-pack@1.0.0` resolves successfully from that same root for its
   compatible `python-cli` profile.

Run the focused `pack_actions` integration test first, followed by formatting,
Clippy, and the full Rust test suite because shared CLI discovery behavior is
changed.
