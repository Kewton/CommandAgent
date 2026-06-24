# Test0620 005 Plan Block Scalar Contract

Date: 2026-06-21

## Problem

Gemini `/ultra-plan-run --profile nextjs` stopped before phase execution with:

```text
ERROR: invalid plan YAML: unsupported block scalar style for instruction: >-
```

The ultra plan was generated, but the generated phase step plan failed while
being parsed or corrected. The minimal loop did not start for that phase.

## Layer Classification

Responsible layer:

- Planning Contract
- plan-file parser / normalizer

Non-responsible layers:

- provider transport
- minimal loop execution
- Next.js profile verification
- verifier/setup policy

## Root Cause

CommandAgent documentation and prompts said that public plan inputs may use
YAML block scalars for known long text fields. The parser accepted only exact
`|` and `>` markers, while ordinary YAML commonly emits chomping indicators
such as `>-` or `|-`.

The failure chain was:

```text
public plan contract allows block scalars
  -> planner emits instruction: >-
  -> parser rejects >-
  -> bounded correction spends attempts on syntax that can be normalized
  -> correction exhausts before execution
```

This is not Gemini-specific behavior. The fix belongs at the provider-neutral
plan input boundary.

## Fix Direction

Known long text fields now accept:

- `|`
- `|-`
- `|+`
- `>`
- `>-`
- `>+`

Supported fields:

- step plan `goal`
- step `instruction`
- ultra plan `goal`
- ultra phase `goal`

The parser normalizes these markers into CommandAgent's typed string
representation. Exact trailing-newline chomping is not part of the public
behavior contract.

## Verification Criteria

The targeted success criterion is not a complete Next.js app. The criterion is:

- `instruction: >-` no longer causes `unsupported block scalar style`
- bounded correction is not exhausted solely because of that syntax form
- later failures, if any, are classified by their responsible layer

## Checks To Record After Implementation

- focused parser tests for `|-`, `|+`, `>-`, and `>+`
- step plan parse/render/reparse tests
- ultra plan parse/render/reparse tests
- offline `/run-plan` E2E with `goal: >-` and `instruction: >-`
- focused Gemini `/ultra-plan-run` UAT moving past the previous parser failure

## Implementation Verification

Local checks:

```text
cargo test yaml_scalar                                      passed
cargo test accepts_folded_strip_block_scalar_instruction    passed
cargo test accepts_literal_strip_block_scalar_goal_and_canonicalizes passed
cargo test accepts_folded_strip_block_scalar_ultra_goal     passed
cargo test accepts_literal_strip_block_scalar_phase_goal    passed
cargo test plan_correction                                  passed
cargo fmt --check                                           passed
cargo test                                                  passed
git diff --check                                            passed
cargo build --release                                       passed
```

Offline `/run-plan` E2E:

```text
workspace: /private/tmp/commandagent-test0620-005-e2e
plan: /private/tmp/commandagent-test0620-005-e2e/block-scalar-plan.yaml
result: passed
output:
  step plan: 1 steps
  step 1/1 verify-readme: running
  step verify-readme: ok
```

Focused Gemini `/ultra-plan-run` E2E:

```text
workspace: /private/tmp/commandagent-test0620-005-gemini-fresh
provider/model: gemini / gemini-3.1-flash-lite
planner: gemini-3.5-flash
saved ultra plan: .commandagent/plans/ultra-plan-1782025443730-3011nextjs.yaml
result: moved past the previous block-scalar parser failure
next failure layer: Planning Contract / Next.js Tailwind plan lint
next failure reason: nextjs_tailwind_plan_contract missing tailwindcss, autoprefixer
```

The targeted failure did not recur:

```text
unsupported block scalar style for instruction: >-
```

The remaining Gemini failure is a later plan-lint/profile-obligation issue.
One run reported:

```text
step `create-global-css` mentions Tailwind, but the plan does not make the
Tailwind package/config setup contract complete: missing autoprefixer
```

The fresh workspace rerun produced the same class of later failure:

```text
step `create-styles-and-layout` mentions Tailwind, but the plan does not make
the Tailwind package/config setup contract complete: missing tailwindcss,
autoprefixer
```

That later failure is outside this parser contract slice and should be handled
by the manifest/Tailwind plan-correction layer, not by weakening the parser or
increasing retry count.
