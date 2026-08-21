# Issue 217 design

## Context

Issue #217 is the Epic #260 Lane D combined CLI change for #217, #219, and
#225, plus the additive argument surfaces delegated by #248. The required
#234 predecessor is already incorporated: this worktree and
`feature/issue-234-perf-think-step-planner-think-false-ultra-planne` both start
at `f848084e`.

The current Clap definition exposes 54 public application flags, but 24 have
no help text and several remaining descriptions differ from the English CLI
reference. Help is a flat list headed by the obsolete MVP description. Numeric
budgets accept zero, generated completions are static, and the CLI has no
standalone config-template action. Lane I owns the manifest backend, so this
change must not implement manifest validation or profile generation.

## Design

1. Make the English CLI reference's Description cells the public wording
   contract. Give every visible application flag the same Clap help string,
   replace the command summary, and add five explicit help headings. Extend the
   doc-drift test to require help on every public flag and exact English
   description equality. Keep the Japanese table synchronized semantically.
2. Use positive-integer Clap value parsers for `--max-iterations` and
   `--chat-timeout-secs`, rejecting zero before configuration or provider setup.
   Keep every existing nonzero value and omission behavior unchanged.
3. Enable clap_complete's dynamic engine. Attach a local-model completer to
   `--model` and `--planner-model`, querying the default Ollama `/api/tags` and
   LM Studio `/v1/models` endpoints with a short bound. Merge, sort, deduplicate,
   and prefix-filter candidates. Ignore all client, transport, status, and
   parsing failures so offline completion has no diagnostic output. Preserve
   `--completions <SHELL>` while making its generated registration delegate
   completion back to the installed binary.
4. Add `--init-config` as a direct action. It creates
   `.commandagent/config.toml` below `--cwd` (or the current directory) from a
   valid starter preset, uses create-new semantics, and never overwrites an
   existing config. Put the filesystem behavior in a CLI leaf module and keep
   top-level runtime wiring minimal.
5. Add `validate_manifest: Option<PathBuf>` for
   `--validate-manifest <PATH>` and `init_profile: Option<String>` for
   `--init-profile <ID>`. Require `--extension-root` for initialization and add
   only parsing/help/docs/tests; Lane I will own execution and manifest files.

## Compatibility and scope

- Existing flag spellings, defaults, hidden integration flags, action
  implementations, event schemas, and `.anvil/` state remain unchanged.
- The legacy `--completions` invocation remains the installation interface.
- No Lane I loader, schema, manifest, corpus, or backend file is edited.
- The #234 `Config` initializer changes are preserved; no predecessor commit is
  rewritten.

## Verification

Run focused CLI unit, doc-drift, artifact, and parse tests first. Because the
shared CLI definition, generated artifacts, and binary startup path change,
then run `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, and the full `cargo test` suite.
