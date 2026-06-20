# Plan Update 02 Plan File Contract

Date: 2026-06-20

## Problem

UAT `workspace/mvp/uat/test0620_003.md` stopped before execution:

```text
ERROR: invalid plan YAML: unexpected line:
    Create the package.json file and configure the dev script to use port 3011.
```

The saved invalid phase step plan used an ordinary YAML block scalar:

```yaml
instruction: |
  Create the package.json file and configure the dev script to use port 3011.
  The package.json work must mention ...
```

This was valid YAML, but CommandAgent's line-oriented step-plan parser did not
support block scalar strings. The failure belongs to the Planning Contract /
plan-file parser layer. The minimal loop and tool execution did not start.

## Design Decision

Plan files are now treated as public contract inputs. A plan may come from the
built-in planner or a future external planner surface. CommandAgent owns:

```text
parse supported plan-file syntax
  -> normalize to typed plan structs
  -> schema validation
  -> plan lint/profile obligations
  -> execution
```

This is not full YAML-as-language support. The accepted addition is limited to
ordinary scalar forms for known plan string fields:

- step plan `goal`
- step `instruction`
- ultra plan `goal`
- ultra phase `goal`

Anchors, aliases, merge keys, custom tags, environment expansion, and arbitrary
nested maps remain outside the contract.

## Implementation Summary

Code changes:

- Added a small shared block-scalar helper under the step runner.
- Step plan parsing now accepts `|` and `>` for top-level `goal` and step
  `instruction`.
- Ultra plan parsing now accepts `|` and `>` for top-level `goal` and phase
  `goal`.
- Generated step-plan header normalization now drops block-scalar continuation
  lines when replacing model-supplied top-level headers with CommandAgent-owned
  context headers.
- Canonical renderers remain stable; parsed block scalar input renders back to
  CommandAgent-owned quoted scalar YAML.
- Plan generation and correction prompts now describe the supported scalar
  forms and rejected complex YAML features.

No correction or repair budget was increased. No provider/model-specific branch
was added.

## Focused Tests

Focused tests run during implementation:

```text
cargo test step_runner::tests                  passed
cargo test ultra_plan                          passed
cargo test yaml_scalar                         passed
cargo test invalid_phase_step_plan             passed
cargo test plan_correction                     passed
cargo test generated_header_normalization_skips_block_scalar_goal_body passed
```

These tests cover:

- `instruction: |`
- `instruction: >`
- top-level step-plan `goal: |`
- ultra-plan `goal: |`
- ultra phase `goal: |`
- ultra phase `goal: >`
- generated step-plan header normalization when model output includes
  top-level `goal: |`
- parse -> canonical render -> parse
- bounded phase-step correction where corrected YAML uses `instruction: |`

## Verification

```text
cargo fmt --check                                      passed
cargo test                                             passed
python3 tests/test_eval_report.py                      passed
python3 -m py_compile scripts/eval_report.py           passed
cargo clippy --all-targets -- -D warnings              passed
cargo build --release                                  passed
scripts/check_branding.sh                              passed
git diff --check                                       passed
```

## Focused UAT

Focused local UAT used a hand-written step plan file with block scalar values in
both top-level `goal` and step `instruction`, then ran:

```text
target/release/commandagent --offline "/run-plan block-scalar-plan.yaml"
```

Result:

```text
step plan: 1 steps
step 1/1 verify-readme: running
step verify-readme: ok
```

This confirms the saved plan-file parser path moves past the previous
`invalid plan YAML` failure class for `instruction: |`. A Gemini Next.js UAT was
not rerun in this slice because the change targets the provider-independent
plan-file parser boundary; later Next.js quality, dependency setup, or profile
verification failures should be classified separately by layer.

## Interpretation

This change should resolve the observed parser failure class. It does not claim
that the generated Next.js app is complete, buildable, visually polished, or
gameplay-complete. Those remain profile, verifier, dependency setup, or quality
obligations.
