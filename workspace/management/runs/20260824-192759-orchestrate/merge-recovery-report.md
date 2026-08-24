# Merge Recovery Report

## Scope

- Issue: #370
- PR context: #379
- Branch: `feature/issue-370-gui-trial`
- Pre-merge head: `9e8e178b97b49c78411ad9d2ba1783168227cdd9`
- Exact base: `7abad1484fa29051a692f0b452b8158bb68808e2`
- Merge commit: `b9864ee82fd5d87d5fcb79734afe5e93b4464d03`
- Merge parents: `9e8e178b97b49c78411ad9d2ba1783168227cdd9` and `7abad1484fa29051a692f0b452b8158bb68808e2`

The sync used a normal merge commit. No rebase, history rewrite, push, PR or
Issue mutation, CommandMate lifecycle action, or historical-run modification
was performed.

## Conflict resolution

- `README.md` and `README.ja.md`: combined Issue #370's separate Trial page
  wording with Issue #371's four-layer extension guide and lifecycle links.
- `docs/guide/README.md`: kept the Issue #370 route description and Issue
  #371's English/Japanese extension entry points.
- `docs/user/gui-help-map.md`: kept the Issue #370 four-page vocabulary and
  Issue #371's extension layer/reference terms.

No code conflict required manual resolution. The post-merge audit retained
Issue #369's deterministic idle completion-boundary test and exact provider,
model, and intent assertions; Issue #370's four-route/session-index behavior;
Issue #371's extension-root work; and Issue #375's plan-step event behavior and
corpus coverage. Gate 1, honest-failure, event schema, and read-only semantics
remain covered by the passing tests.

## Verification

- Status: `passed`
- Exact CI race regression: `passed`
- Full GUI-server suite (40 tests): `passed`
- Read-only guard (26 tests): `passed`
- Doc-drift suite (23 tests): `passed`
- Root/proxy route and session-index smoke: `passed`
- Root/proxy feedback smoke: `passed`
- GUI syntax, lint, typecheck, and production build: `passed`
- Rust format and default/GUI warnings-denied Clippy: `passed`
- Full `cargo test`: `passed`

Exact commands and results are recorded in
`dev-reports/issue-370/verification.md`.

## Final ancestry

The final evidence commit is a direct first-parent child of merge commit
`b9864ee82fd5d87d5fcb79734afe5e93b4464d03`. That merge commit's second parent
is the required exact base `7abad1484fa29051a692f0b452b8158bb68808e2`.
