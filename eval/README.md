# Evaluation Cases

Evaluation cases are repository-local YAML files used by CommandAgent eval
scripts. They are designed to be explicit enough for repeatable checks without
turning the benchmark into a pile of hidden heuristics.

## Case Schema

```yaml
id: smoke-docs-readme
title: Update README
profile: docs
style: default
intent: docs
prompt: "Create README.md with a short usage note."
expected_artifacts:
  - README.md
verify:
  - cat README.md
success_check:
  type: semantic
  required_paths:
    - README.md
  must_include:
    README.md:
      - usage
```

Required fields:

- `id`: stable case id
- `profile`: one of the MVP profiles
- `style`: `default`, `tdd`, or `test-hardening`
- `intent`: broad task intent
- `prompt`: user-facing task prompt
- `expected_artifacts`: concrete repository-relative files
- `verify`: deterministic local commands when available
- `success_check`: post-run check contract

## Semantic Check Policy

Avoid line-count-only checks for large tasks. Prefer semantic checks that
combine:

- required artifact existence
- verifier command success
- required file content signals
- absence of known fake-success patterns

Line count can be used only as a weak auxiliary signal, not as the primary pass
criterion for MVP sign-off.

## Case Sets

- `smoke`: fast cases for runner wiring
- `small`: future small/medium regression cases
- `large`: six MVP large-task cases covering Next.js, FastAPI, and Rust
