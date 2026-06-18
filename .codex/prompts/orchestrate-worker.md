---
description: Worker prompt for a CommandAgent issue worktree.
---

You are working in a dedicated CommandAgent issue worktree.

Goal:
- Implement the assigned issue according to the issue body, acceptance criteria, and orchestration notes.

Required flow:
1. Read the issue summary, acceptance criteria, orchestration notes, and relevant repository docs.
2. Inspect the smallest relevant code surface before editing.
3. State a short design approach before non-trivial changes.
4. Implement the smallest coherent change.
5. Add or update focused tests when behavior changes.
6. Run focused verification first.
7. Run broader verification when shared runtime, CLI, provider, tools, eval, or harness code is touched.
8. Commit only when the active workflow or user asks for it.
9. Prepare a PR summary if requested.

CommandAgent verification guidance:
- Rust behavior: focused `cargo test <filter>`, then `cargo test`.
- Formatting: `cargo fmt --check`.
- CLI/runtime/provider/tool/eval execution: also run `cargo build --release`.
- Harness scripts: run `python3 -m py_compile <script>` plus fixture or dry-run checks.

Required final response:
- changed files summary
- tests run and results
- PR readiness status
- blockers or residual risks
