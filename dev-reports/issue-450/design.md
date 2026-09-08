# Issue #450 design

Add a standalone Browser preflight under `scripts/browser_preflight/` with a
small CLI, fixtures and focused tests under `scripts/tests/`. Leave the frozen
0908 harness, integration worktree, plugin cache, product runtime and Rust event
contracts untouched. No predecessor Issues were assigned; the issue branch is
clean at the start of this work.

## Contract

- Resolve plugin entry points only from an explicit current-session instruction
  attestation (session ID, skill path and content hash). Check the exact referenced
  entry point and record installed versions as inventory, never as candidates.
  Missing provenance is unknown; missing files and operation errors stay distinct.
- Reserve attempts in a durable, locked session ledger *before* browser work.
  Reuse a fresh successful result and its browser binding; allow at most two
  attempts under unchanged conditions, including interrupted attempts. Changed
  conditions require an explanation, saved with both fingerprints. A reason alone
  cannot bypass the unchanged-condition limit. Failures remain scoped to the
  observed session; other-session evidence is retained without implying a global
  outage or a repaired plugin.
- Provide a plugin adapter that takes an existing Browser handle in the supported
  Node tool, with no bootstrap/reset code. An empty list is recorded, then a new
  task-owned tab may be used. Also provide an explicitly authorized standalone
  Playwright adapter for local, unauthenticated targets using a temporary profile.
  Both save the exact URL, readable state, a declared harmless action and observed
  effect, and a decodable screenshot. HTTP is diagnostic evidence only.
- Freeze a new immutable Browser manifest only after validating artifacts and
  complete browser/version/headless/viewport/locale/timezone conditions. A guarded
  launch validates the same request, fresh evidence, latest session outcome and
  artifact hashes before executing a supplied command. Reserve launch before
  execution; changed conditions or repeated launch require a new campaign.
  Existing campaign harnesses are not silently wired to this gate.
- Treat evaluator input/adapter receipts as trusted local observations, not a
  security attestation. Recompute decisions from observations, hash and validate
  saved artifacts, and fail closed on missing/malformed/stale evidence.

## Verification

Fixture cases cover unknown provenance, missing historical entry point, existing
files with operation failure, HTTP-only success, empty tabs, screenshot failure,
and full operational success. Tests also cover retry reasons, restart/pending
attempt suppression, healthy reuse, unauthorized fallback, manifest tampering,
changed conditions and prevention of command execution when the gate fails.
Run focused pytest, Ruff and broader Python regression checks appropriate to the
new harness surface. Save local GUI smoke evidence for the current Browser
connection and authorized isolated-profile fallback, using only a task-owned
loopback fixture server, tabs and temporary browser resources. No model generation
is needed for smoke. Report automatic diagnosis implemented separately from plugin
permanent repair (not implemented or established by this Issue).

## Read-only references inspected

Integration worktree `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`:
`AGENTS.md` Browser reliability; `workspace/tmp/0908/result/browser-recurrence-prevention-20260909.md`;
R0 stop report and review resolution; alternative-browser `validation.json`; and
the existing campaign harness's immutable manifest/pins checks. The historical
failure names deleted version `26.818.21641`; the current session's Browser skill
names `26.825.51511`. The latter entry point exists and initial connection in this
session succeeded; neither observation establishes a permanent plugin repair.
