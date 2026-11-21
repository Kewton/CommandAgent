# G1 Landing Criteria — 2026-10-22

[Model probe guide](../model-probe.md) | [Scenario suite](../../dev/uat/scenarios.md)

This record fixes, before measurement, how the G1 generality campaign's full
scenario round (stage 3a) will be run and judged. It follows step 3 of the new
model procedure in the model probe guide ("commit landing criteria before the
full scenario round"). It states measurement rules only. It is not a result, a
tier-table entry, or a capability claim.

## Measurement identity

- Code under test: the `develop` commit that merges PR #815 (Issue #810). The
  exact SHA, the `git archive` SHA-256 and the release binary SHA-256 are
  recorded in the campaign summary before the first run. The binary is built
  from a `git archive` export, so `--version` prints `unknown` for the commit;
  the two hashes identify the build instead.
- Host: macOS (Darwin, arm64), 256 GB memory, internal disk only.
- Provider and host for every role: `ollama` at `http://127.0.0.1:11434`.

| Role name | Exact model ID | Ollama digest | Route |
| --- | --- | --- | --- |
| Q | `qwen3.8-flash-next:125b-mlx` | `c7b96ffe21af` | local (MLX runner) |
| G | `gemma4:31b-cloud` | `ef09f235533c` | local Ollama forwarding to a cloud model |

G is a cloud model and its identity cannot be pinned. Its result is valid only
with the campaign date and the probe cards below; `/api/chat` returns the model
name `gemma4:31b` for this tag.

## Configuration

Each run pins all three roles in a complete preset, with no inheritance.
Executor/planner/classifier:

| Arm | Executor | Planner (P0) | Classifier (C0) |
| --- | --- | --- | --- |
| Q | Q | G | Q |
| G | G | G | Q |

Shared budget for every arm: `api = "chat_completions"`,
`tool_protocol = "native"`, `context_budget = 65536`,
`chat_timeout_secs = 600`, `planner_think = "false"`, no `--think`,
`plan_preset = "none"` (the value the default resolves to for this planner),
and `--recovery-plan-auto-runs 0` (no automatic recovery).

The command shape for each run, from a fresh empty working directory outside
the checkout:

```text
commandagent --yes --preset <arm preset> --ollama-host http://127.0.0.1:11434 \
  --recovery-plan-auto-runs 0 --ultra-plan-run [--profile <profile>] "<goal>"
```

This is the non-interactive flag form of `/ultra-plan-run`.
The process environment carries no API keys and does not load `.env`.

## Why this configuration (stages 1–2 summary)

- **v3 probes.** Ten `model-probe-v3` invocations ran, all exiting 0:
  - **Executor.** Q completed 10/10 in all 4 runs. G completed 9/10 in all
    6 runs; `edit_own` failed with a repeated `edit_anchor_not_found`.
  - **Planner.** Both models wrapped the JSON in a code fence, so the probe
    scored `failed` in all 10 runs. The JSON inside was valid in all 10 runs,
    and the production StepPlan parser accepts that form.
  - **Classifier.** G fenced its answer in 6/6 runs, and the production
    classifier rejects fences, so G is not used as classifier. Q answered with
    bare JSON in 3/4 runs.
- **Smoke runs.** Four smoke runs (CLI and TOOL × two executors) ended
  honestly; there was no false full success. The CLI run with executor Q was
  `full_success`, and a human audit confirmed it. Both TOOL runs stopped
  during planning at the 2,500-character step limit or on a weak verify
  command. The first of these is fixed by #810.
- **Classifier calls.** The classifier was not called in any smoke run, because
  each passed an explicit `--profile`.

## Scenarios and order

The five scenarios are the original Japanese prompts in
[`docs/dev/uat/scenarios.md`](../../dev/uat/scenarios.md), used verbatim:
GAME, TOOL, CONTENT, CLI and AMBIGUOUS.

- GAME, TOOL and CONTENT use `--profile nextjs`.
- CLI uses `--profile python-cli`.
- AMBIGUOUS runs with no `--profile` flag, and no stack or profile token is
  added to its prompt.

There are 10 runs: 5 scenarios × 2 arms, one run each. The arm order
alternates by scenario to reduce order effects:

| Scenario | Order |
| --- | --- |
| GAME | Q → G |
| TOOL | G → Q |
| CONTENT | Q → G |
| CLI | G → Q |
| AMBIGUOUS | Q → G |

The runs are serial, inside one machine-wide heavy slot, each in a new empty
working directory. Before every web-gated run, port 3011 must have no listener.
Before every run, internal-disk free space must be at least 12 GiB.

## Scoring

- **Per requirement.** Each run is scored per requirement as `pass`, `fail` or
  `unverified`. A person scores it from the produced artifacts and the
  execution evidence (events, probe evidence, and running the produced program
  or app). The scenario's "Final acceptance requires" and "Partial means"
  sections are the requirement list. A model's own claim, an HTTP 200, a
  successful build or a keyword in source does not count as meeting a
  requirement.
- **Full successes.** Every `full_success` is audited against the user
  requirements. A full success that does not meet them is a false full, which
  stops the campaign.
- **Non-full runs.** These record the first failing stage and its signature, and
  whether each gate was reached. A gate never reached is not counted as a
  downstream capability failure.
- **#810.** On every `nextjs` run the events must show:
  - no `recovery_verifier_plan_admission` rejection for the 2,500-character
    capacity;
  - the short profile-contract reference in the owning step's instruction;
  - the full contract under `Profile guidance:` in the system prompt.

## Denominators

Report all of these counts side by side:

- planned;
- not started;
- started;
- not judgeable for environmental reasons;
- judgeable;
- reached the given check.

Report separately, over judgeable runs:

- requirement satisfaction;
- full rate;
- check-reach rate;
- pass rate after reaching a check;
- honest-terminal rate;
- false-full count;
- the distribution of failure signatures.

Also keep the operational success rate over all started runs. Counts are given
per scenario × arm, so one scenario cannot hide another.

**Every cell has n<10, so this round declares no ranking, no superiority and no
significance between the models or arms.** A difference of 1/1 against 0/1 is
recorded as an observation, not as a verdict.

## Limits, replacement and stop rules

- **Outer limit.** The outer wall clock is 45 minutes per run. A run stopped at
  this limit is recorded as a harness cutoff and kept apart from the product's
  own honest-terminal rate.
- **Replacement.** Only an interruption shown by evidence to be environmental
  (host, disk, network or service fault) may be replaced: at most once per
  planned run, and at most 4 times in the whole round. The replacement gets a
  new run ID with `replacement_of`, and the original run is kept. Invalid model
  output, unimplemented requirements and normal budget exhaustion stay in the
  denominator and are not replaced.
- **Stop immediately and report** on any of these:
  - a false full;
  - a secret leak;
  - loss of mutual exclusion;
  - internal-disk free space under 12 GiB;
  - an Ollama failure;
  - a failure to load the 125B model;
  - a cloud 429, quota or billing notice;
  - a finding that requires a code change.
- **Repeated defect.** If the same mechanism defect appears in two consecutive
  runs, stop before the remaining runs and report it.
- **Fixed during the round.** Code, prompts and protocols are not changed
  during the round. Changing any of them starts a new campaign; this record is
  not overwritten.
