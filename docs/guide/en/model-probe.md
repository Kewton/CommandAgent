# Model Behavior Probe

[日本語](../ja/model-probe.md) | [Guide index](../README.md)

`commandagent --model-probe` and the TUI `/model-probe` command run a
bounded dialect probe against the configured executor, planner, and classifier
model roles. The classifier role is configured through a selected preset.
The probe is measurement-only: results never auto-configure runtime behavior.
The output is a JSON profile plus a markdown card for human review and
model-tier table evidence.

The probe uses a throwaway scratch workspace under the system temp directory,
cleans it after the run, and must not run package installs. Each task is one
bounded session through the normal model/tool chokepoint unless noted below.
The fixed battery version is `model-probe-v3`.

## Battery

1. `write_simple`: create `src/util/math.ts` with a 5-line function.
   Measures relative, absolute, and corrupted path forms.
2. `write_deep`: create a file 5 directories deep with a long filename.
   Stresses path reconstruction.
3. `edit_provided`: edit a file whose exact content is supplied in the prompt.
   Measures anchor-copy fidelity.
4. `edit_own`: edit the file written in task 1 in the same session.
   This is appended-session variant A for memory-anchor fidelity.
5. `verify_exist`: verify that `src/util/math.ts` exists.
   Measures command shape such as `test -f`, `&&`, pipes, and `cd`.
6. `verify_json`: verify that `package.json` declares a build script.
   Measures JSON-verification command dialect such as `node -p`, `grep`,
   pipes, redirects, and shell control.
7. `repair_appended`: repair a planted one-line compile error in a session
   with roughly 2k tokens of prior context.
8. `repair_compact`: repair the same planted error in a fresh minimal session.
   The appended-vs-compact delta is the context-sensitivity signal.
9. `regenerate`: rewrite the full corrected file via `Write`.
   This checks the instruction-96 full-file regeneration capability.
10. `csv_fixture_verify`: create a small CSV fixture, then verify a local
    program can process it.
    This elicits the live combine tendency where models try to create fixture
    files inside verify commands with redirects or heredocs.
11. `json_schema`: respond only with JSON matching the StepPlan-like schema.
    This measures planner parse rate and missing descriptive or semantic fields.
12. `classifier_closed`: select one matching route from a fixed closed list and
    return exactly the three classifier keys. This uses the configured
    classifier model, `think=false`, and the production classifier generation
    cap without changing runtime configuration.

Every task records raw tool calls and raw Bash commands verbatim in the JSON
profile.

## Metrics

The profile computes:

- per-role fixed-probe completion bands (`complete`, `partial`, or `failed`),
  provider duration, turn count, latency, and token telemetry for executor,
  planner, and classifier; the band is not a production capability tier

- `absolute_path_rate` and `corrupted_path_count`
- `shell_control_rate` with `&&`, `;`, pipe, redirect, and `cd` breakdowns
- `edit_anchor` as `exact`, `salvageable`, or `miss`
- `repair_follow_through` for appended and compact repair
- `regeneration_follow_through`
- `json_valid_rate` and `missing_field_kinds`
- `classifier_valid_rate`
- `empty_response_rate`
- `malformed_tool_call_rate`
- per-call latency stats, plus a first-turn versus later-turn latency note for
  cache-effect visibility within the same provider/model run
- provider token telemetry from the 98C provider-turn events, including
  context-truncation warnings when they fire

The markdown card maps elevated metrics to the absorbing layers they exercise:
path normalization/salvage, bash and verify normalization, edit-anchor salvage,
compact repair rungs, full-file regeneration, schema repair/defaulting, empty
response retries, malformed tool-call recovery, and token-truncation review.

The card states its own scope: N=12 micro-tasks, dialect indicators, not a
capability benchmark.

Calibration note: `model-probe-v1` did not include a fixture-creation plus
verification task, so it could under-sample the live python-cli tendency to
combine CSV fixture creation and verification in one shell command. `v2` records
that divergence honestly by adding `csv_fixture_verify`; old v1 cards remain
valid for their stated N=10 dialect scope but should not be treated as evidence
about redirect/heredoc fixture creation behavior.

`model-probe-v2` also had no classifier task or per-role timing table. Its
N=11 cards remain valid for their stated aggregate scope, but they cannot be
used as classifier evidence or compared directly with the v3 N=12 band.

## Role-Pair Procedure

Use a complete preset so the classifier does not silently inherit a different
planner value. Pin exact model IDs and keep provider, digest, context budget,
thinking setting, tool protocol, CommandAgent build, and host constant across
arms. For example:

```toml
[preset.role_pair_probe]
model = "<executor-model-id>"
provider = "ollama"
api = "chat_completions"
tool_protocol = "native"
planner_model = "<planner-model-id>"
planner_provider = "ollama"
planner_think = "false"
classifier_model = "<classifier-model-id>"
classifier_provider = "ollama"
context_budget = 65536
chat_timeout_secs = 600
profile = "generic"
narration = "quiet"
footer = "off"
stream = "off"
prompt_layout = "legacy"
plan_preset = "none"
```

Run each arm at least twice and inspect model residency before interpreting a
first split-model duration:

```bash
commandagent --preset role_pair_probe --model-probe
ollama ps
commandagent --preset role_pair_probe --model-probe
```

Compare a same-model baseline with one role changed at a time. Use the per-role
duration, not the aggregate battery duration: executor task-count or retry
variation can otherwise hide the role being tested. A smaller model is not
automatically faster, and a `complete` micro-probe band does not replace smoke
or full-scenario acceptance.

## Current Measured Local Recommendation

The 2026-08-22 local measurement supports `qwen3.8:27b-mlx` for executor and
planner, with `qwen3.5:4b` only for the classifier. The independent 4B
classifier completed all four observed classifier tasks in the relevant arms;
the final hybrid measured 176–304 ms. No smaller planner is recommended: the
warm 9B planner was slower than the 27B baseline, and the 4B planner met its
JSON contract in only one of two runs.

See the [full measured record, exact digests, durations, and
checksums](../model-probe-results/2026-08-22-local-role-pairs.md). This is a local
probe/smoke starting point, not a built-in default or a universal tier claim.

## New-Model Procedure

Standard order before scenario UAT:

1. Run `commandagent --model-probe` or `/model-probe` with the intended
   executor/planner/classifier configuration and review the generated card.
2. Run two smoke checks: one CLI task and one TOOL task.
3. Run the full scenario round with landing criteria committed before the run.
4. Add the tier-table entry with a citation to the probe profile.

Re-run the probe when the model version or digest changes. For cloud-hosted
models, re-run it before every measurement campaign because identity is not
pinned.
