# Issue #428 design

## Evidence and scope

- Read `AGENTS.md`, `docs/dev/dev-guardrails.md`, and the issue-worker skill.
- Inspected the current [Issue #428 body and comments](https://github.com/Kewton/CommandAgent/issues/428) with `gh issue view`; there were no comments. The worktree starts at the reported reproducer revision, `0296d779eed87f244899199a5687f8261b449e3f`, with no pending changes or required predecessors.
- `absolute_path_candidates` scans raw command text, treating `]` as a new absolute-path boundary and omitting shell separators from path endings. The write guard already tokenizes redirects and allows exactly `/dev/null` for output redirection.

## Change

Add a private leaf helper for command path candidates. Decode shell word quotes and escapes, keep adjacent quoted fragments in one word, and split unquoted redirects and command separators. Recognizable literal path words retain their entire path, including dynamic-route brackets. Keep conservative embedded-path scanning for command strings, assignments/options, substitutions, and syntax that cannot be classified as a literal path. Malformed quoting must not silently disable scanning. Keep diagnostic-output scanning separate from command tokenization.

Wire this helper only into Bash command confinement. Preserve the existing system-prefix policy, write-target validation, authority checks, event names/schema, and Recovery acceptance/promotion logic. Do not change runtime state, historical evidence, guardrail baselines, or unrelated documentation.

Full-path safety review: once a relative route is no longer rejected as a bogus absolute suffix, its complete path must still remain inside the workspace. Reuse `path_guard::ensure_bash_write_target` as the existing path proof for literal relative references and workspace absolute references, including parent traversal and symlink/missing-leaf resolution. Keep conservative inspection for unquoted glob patterns and multiword executable payloads so literal spelling does not authorize an expanded outside path. This requires no new filesystem allow-list or change to the write guard.

The referenced local evidence index is absent from this worktree; the current Issue body provides the reproducing commands and event details. Historical/external evidence remains untouched. Per active-batch coordination, do not edit `tests/corpus_regression.rs`; a dedicated integration test consumes the fixture, and a leaf test attached under `auto_recovery`'s existing test module exercises the real promotion gate.

## Verification plan

Add focused lexer and tool-registry tests for quoted, escaped, and joined dynamic routes (including approve/reject), `/dev/null` redirects with adjacent separators, and real outside-root references in ordinary and embedded syntax. Exercise traversal, existing symlink escape, unauthorized writes, and the exact `/dev/null` exception. Add a source-only E1-style expense Recovery corpus fixture and exercise its reads in a temporary treatment workspace; confirm failed build/business acceptance still cannot promote using the existing Recovery gate tests.

Run focused Bash and corpus/Recovery checks, then `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`. Record actual outcomes before committing only task-owned source, tests, and reports. No push or external state mutations.
