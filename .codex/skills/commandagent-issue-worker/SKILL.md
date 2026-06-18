---
name: commandagent-issue-worker
description: Implement one assigned CommandAgent issue or task in a dedicated git worktree. Use when Codex is working from orchestration notes, issue acceptance criteria, or a feature branch and needs a scoped implementation workflow for this repository.
---

# CommandAgent Issue Worker

Use this skill inside a dedicated CommandAgent issue worktree or feature branch.

## Required Flow

1. Read the issue, acceptance criteria, orchestration notes, and relevant docs.
2. Inspect the smallest relevant code surface before editing.
3. State the intended design in the conversation or a requested planning artifact before non-trivial edits.
4. Implement the smallest coherent change inside the responsible layer.
5. Add or update focused tests when behavior changes.
6. Run focused verification first, then broaden based on risk.
7. Summarize changed files, tests run, residual risks, and PR readiness.
8. Commit only when the user asks for a commit or the active workflow requires it.

## Repository Rules

- Follow `AGENTS.md` and `docs/development.md`.
- Keep provider modules transport-only.
- Keep the minimal loop focused on one execution session.
- Keep planning, verification, repair, and eval as explicit contracts.
- Do not add hidden retry loops, sidecar routing, legacy engines, case memory, Photon/PAM behavior, or provider-specific policy branches.

## Verification

Prefer the narrowest command that proves the change:

- Rust behavior: focused `cargo test <filter>`, then `cargo test`.
- Formatting: `cargo fmt --check`.
- CLI/runtime/provider/tool/eval execution: also run `cargo build --release`.
- Harness script changes: run script compile plus fixture or dry-run checks.

Use live providers or network only when the user explicitly asks.
