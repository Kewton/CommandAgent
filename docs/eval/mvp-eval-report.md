# MVP Eval Report

Date: 2026-06-17

## Scope

This report records the first MVP eval sign-off pass for CommandAgent using the
release binary. It covers:

- offline smoke wiring
- real `/plan-run` smoke eval roots
- preliminary large-task eval
- meta.json traceability checks

Live interactive UAT is recorded separately in `docs/eval/mvp-uat.md`.

## Binary And Metadata

The clean smoke roots below used:

- binary: `target/release/commandagent`
- mode: `plan-run`
- commit: `21a2ab8aefa87bf7cc5fc028625967db41f3a7b8`
- dirty: `false`

The preliminary large root was produced earlier during migration:

- commit: `205ea7358e2adb96b9efc9f05bbe50b86b28c578`
- dirty: `true`

Because the large root is dirty and predates the final plan-run eval wiring, it
is useful for triage but not for release-quality comparison.

The latest large root after per-case mode, fixtures, and YAML drift fixes used:

- root: `eval/runs/mvp-large-fresh/20260617T003924`
- commit: `49ffc3b63a73505ec406d6bda9c94d29a0196d10`
- dirty: `false`
- mode: `ultra-plan-run`

## Offline Smoke

`scripts/eval_smoke.sh` passes. This covers formatting, unit tests, release
build, CLI help, branding checks, and dry-run eval wiring.

## Smoke Eval

| Root | Model | Result | Notes |
| --- | --- | ---: | --- |
| `eval/runs/mvp-smoke/20260617T001334` | `qwen3.6:27b-coding-nvfp4` | 1/3 | `docs` passed; `python` and `rust` failed due missing artifacts. |
| `eval/runs/mvp-smoke/20260617T001510` | `qwen3.6:35b-a3b-coding-nvfp4` | 2/3 | `docs` and `rust` passed; `python` failed with `rc=1`. |
| `eval/runs/mvp-smoke/20260617T000015` | `qwen3.6:27b-coding-nvfp4` | 3/3 | Earlier dirty migration run; demonstrates the path can pass, but not a clean sign-off root. |

Interpretation:

- The eval runner now invokes cases through `/plan-run`, matching actual MVP use.
- Clean smoke eval is not yet stable enough to be a release gate.
- The failures are planner/model-output instability and tool-policy interaction,
  not missing runtime dispatch. Live UAT covers the same docs/Python/Rust flows
  successfully with explicit REPL commands.

## Large Eval

| Root | Model | Result | Notes |
| --- | --- | ---: | --- |
| `eval/runs/mvp-large/20260617T000606` | `qwen3.6:27b-coding-nvfp4` | 0/6 | Preliminary only; dirty root, plan-run mode, no fixture seeding for modify cases. |
| `eval/runs/mvp-large-fresh/20260617T002756` | `qwen3.6:27b-coding-nvfp4` | 0/6 | Clean root after mode/fixture support; all cases stopped at ultra-plan YAML parsing. |
| `eval/runs/mvp-large-fresh/20260617T003233` | `qwen3.6:27b-coding-nvfp4` | 0/6 | Clean root after ultra-plan indentation fix; reached execution and repair in several cases. |
| `eval/runs/mvp-large-fresh/20260617T003924` | `qwen3.6:27b-coding-nvfp4` | 0/6 | Clean root after step-list and nested-phase drift fixes; no parser-wide immediate failure remains. |

Observed direct reasons:

- `large-fastapi-app-modify`: model planned inspection of `app/routes/*.py`
  although fixture routes live directly in `app/main.py`; repair prompt saved.
- `large-fastapi-app-new`: plan lint rejected a step that mixed setup/editing and
  verification.
- `large-nextjs-app-modify`: model attempted a blocked compound shell command
  during inspection; repair prompt saved.
- `large-nextjs-app-new`: dependency install was blocked by offline policy, so
  the Next.js app was not completed.
- `large-rust-app-modify`: a phase reached `max_iterations` before creating
  `src/lib.rs`; repair prompt saved.
- `large-rust-app-new`: plan lint rejected a step that mixed tests/editing and
  verification.

This is valid triage data, but not a release-quality benchmark yet.

## Regression Checks

- `goal is too long` did not appear in the recorded clean smoke roots.
- Repair fallback stores bounded packets under `.commandagent/repairs` and
  suggests `$(cat ...)` commands instead of embedding the full failed session as
  a new goal.
- `meta.json` includes case id, run index, provider, model, profile, style,
  mode, prompt, binary path, commit, dirty flag, timeout, elapsed time, return
  code, success, and success-check reason.

## Sign-Off Status

MVP runtime sign-off is partially complete:

- Offline smoke: pass.
- Live UAT: pass for docs, Python, Rust, Next.js file-set, planner/executor
  split, and repair fallback.
- Clean eval smoke: not yet stable.
- Large eval: fresh clean run exists, but result is 0/6 and not release-ready.

Release-quality eval requires follow-up work:

1. Triage the latest large root before adding more runtime control. The highest
   signal issues are plan-lint false positives versus real mixed steps, blocked
   inspection/install commands, and max-iteration exits.
2. Keep `runs=1` as MVP smoke sign-off and use `runs=3` for release-quality
   stability checks.
3. Avoid tuning the planner parser for every model-specific YAML drift unless
   the drift exposes a clear common contract bug.
