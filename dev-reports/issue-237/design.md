# Issue 237 Design

## Problem

CommandAgent trims conversation state against the resolved `context_budget`, but
its Ollama `/api/chat` requests do not set `options.num_ctx`. Ollama can therefore
load a model with a different context window than the budget CommandAgent uses.
Doctor also does not show the resolved context budget, so the effective value is
not visible in setup diagnostics.

## Design

- Initialize every `OllamaClient` with an explicit default `options.num_ctx`,
  alongside the existing `options.num_predict`, and retain the existing public
  constructor signature.
- Pass `Config.context_budget` through the production provider factory. Probe-only
  clients use the same explicit default, although `/api/tags` requests do not use
  chat options.
- Add a `config.context_budget` doctor check with the resolved value and its
  existing source metadata. This keeps doctor output provider-independent while
  making the Ollama `num_ctx` value auditable.
- Update the existing Ollama request byte fixtures and English/Japanese provider
  documentation. No event names, event schemas, runtime state, or conversation
  trimming behavior change.

## Verification

- Focused Ollama provider unit tests, including request-option and fixture
  equality assertions.
- Focused doctor tests for the new check.
- Existing corpus contract tests.
- Repository formatting, Clippy, and full Rust test suite because shared provider
  construction and doctor output are touched.
