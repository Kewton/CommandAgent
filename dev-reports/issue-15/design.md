# Issue #15 Design

## Scope

Replace the remaining directly visible Anvil product branding in the startup
banner, interactive prompt, planner persona, generated interaction-probe package
description, and current user documentation. Preserve all behavior, schemas,
event names, historical evidence, `ANVIL_*` environment variables, `.anvil/`
runtime paths, and compatibility identifiers such as `anvildev` and
`data-anvil-*`.

## Design

- Replace the three-line Anvil lettermark with a compact CommandAgent wordmark
  while retaining the existing five-line banner dimensions, style selection,
  gradient, and rendering flow.
- Change the interactive prompt to `commandagent> ` and update the PTY assertion
  and README example to match.
- Replace only the product-name literals in the UltraPlan system persona and the
  managed interaction-probe package description.
- Update current product prose from Anvil to CommandAgent. Where the acceptance
  scan covers a legacy `data-anvil-*` mention, describe it generically as an
  instrumentation hook without renaming the live compatibility contract.
- Do not modify immutable migration or historical run evidence.

## Verification

Run the banner unit tests and the Issue acceptance scan first. Then run format,
clippy, `cargo build`, `cargo test --quiet`, and the gated TUI PTY test because
the visible Rust CLI surface and its integration assertion change.
