# Issue #19 Design

## Scope

Replace the repository's single developer-heavy README with bilingual English
and Japanese user entry points. Keep the implementation documentation-only:
do not change CLI behavior, provider behavior, configuration discovery, event
schemas, or the live `.anvil/` runtime namespace.

## Current Contracts

- Use the post-Issue #15 product name `CommandAgent` and binary name
  `commandagent`.
- Use `.commandagent/config.toml` in the workspace or home directory as the
  canonical configuration locations. Mention `.anvil/config.toml` only as the
  supported legacy fallback and retain `.anvil/` for live plan, repair, and run
  paths.
- Document only the action selectors and slash commands present in `src/cli.rs`
  and `src/tui/slash.rs`. Use the canonical profile spelling `python-cli` from
  `src/planner/profile.rs`.
- Keep the accurate warning that CommandAgent reads but does not create config
  files or presets.

## Documentation Structure

- Give both READMEs the same section order and information: language switcher,
  badges, value proposition, demo, features, Ollama quickstart, installation,
  usage, configuration, development/security pointers, and MIT license.
- Make the quickstart local-only and copyable. Use `<your-model>` consistently
  and explicitly require readers to replace it with a model that exists in
  their own Ollama installation.
- Add a small `docs/guide/README.md` landing page so the promised full-reference
  links resolve before the broader guide issue lands.
- Move the old UAT, live-provider test, clean release build, symlink, and copy
  validation procedures to `docs/dev/repository-validation.md`.

## Demo Asset

Add `docs/assets/ux-demo.svg`, an animated terminal excerpt based on the actual
offline `commandagent --ux-demo` output. Add a neighboring VHS tape and recording
note that install the local binary and record the same command, with fast mode
available for maintainer iteration. The README embeds the lightweight SVG near
the top while the tape provides a repeatable full terminal recording workflow.

## Verification

Because executable behavior and Rust sources do not change, use focused
documentation checks: compare the two README heading structures, scan documented
commands and paths against their source definitions, validate the SVG as XML,
run `git diff --check`, render the CLI help, and run the existing CLI, slash-help,
and UX-demo unit-test groups. A full Rust suite is not required for this
documentation-only change.
