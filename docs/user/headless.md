# Headless execution

[CLI getting started](getting-started-cli.md) | [CLI reference](../guide/en/cli-reference.md)

Use `--summary-json` with a normal execution command when another process needs
the terminal result. CommandAgent keeps its existing human output and writes one
compact JSON object as the final stdout line after run evidence has been closed.
On `SIGINT`, CommandAgent closes any available direct-command evidence first and
emits the same final JSON line with `status: "interrupted"` and
`exit_code: 130`. An interruption before an event file exists has no summary to
project. Omitting the flag preserves the existing stdout bytes.

```bash
commandagent --yes --no-footer --summary-json \
  --profile nextjs --ultra-plan-run "Create the app" \
  | tee commandagent.log
jq -R 'fromjson? // empty' commandagent.log | tail -n 1
```

## Exit codes

| Code | Meaning |
| ---: | --- |
| `0` | The command process completed. Read `verdict`, `assurance`, and `gate`; process success alone does not upgrade acceptance. |
| `1` | Execution or validation failed. The summary is still the final stdout line when a run was started; read `stop_class`, `stop_reason`, and the evidence paths. |
| `2` | CLI arguments or pack selection were rejected before a run started, so no run summary is available. |
| `130` | The command was interrupted, including by `SIGINT`. When run evidence exists, the final stdout line reports `status: "interrupted"` and `exit_code: 130`. |

## JSON schema

The schema identifier is `commandagent.headless-summary/v1`. Existing scalar
keys are always present; unavailable measurements are JSON `null`. The
additive `provider_usage_by_role` object is empty when the event stream has no
provider turns. The additive `pack` object is omitted when no pack was selected.

| Field | Source and meaning |
| --- | --- |
| `schema_version` | Stable schema identifier. |
| `run_id` | Parent directory name of the persisted `events.jsonl`. |
| `verdict` | Existing final-acceptance verdict; if none was recorded, the same terminal assurance fallback used by the GUI status projection. |
| `assurance` | Terminal event `assurance_level`; one of the values already earned by the product, such as `full`, `partial`, `static`, or `failed`. |
| `score` | Latest persisted score value, including a score-checkpoint vector; `null` when the run has no score evidence. |
| `acceptance_sheet_path` | Existing run-local terminal/acceptance sheet (`summary.md`), when present. |
| `artifacts_dir` | Persisted `run_start.workspace_root`, which is the root containing the generated or repaired artifacts. |
| `events_path` | Exact event-stream path used by the run. |
| `duration_secs` | Existing terminal `time_profile_total_ms`, converted from milliseconds to seconds; not a new wall-clock estimate. |
| `provider_cost_usd` | Persisted provider/run cost when one exists; CommandAgent does not invent prices from tokens. |
| `provider_usage_by_role` | Provider wall time and provider-reported prompt, generation, thinking-token, and prefill-ratio measurements grouped as `planner`, `executor`, `repair`, and `acceptance-repair`. Missing provider-reported measurements are `null`; the ratio is a fraction from `0.0` to `1.0`. |
| `stop_class` | Failed terminal event `failure_kind`; `null` for a successful terminal. |
| `directive_round` | Latest persisted directive round, or `0` for an ordinary non-directive run. |
| `status` | Terminal process status: `completed`, `failed`, or `interrupted`; `null` when no terminal status can be projected. This does not replace `verdict` or `assurance`. |
| `gate` | Existing terminal release-gate status; `null` when no gate was recorded. |
| `stop_reason` | Existing terminal stop, primary, or failure reason; `null` when unavailable. |
| `next_action` | Existing terminal recovery/next-action value; `null` when unavailable. |
| `changed_files` | Sorted, deduplicated Git working-tree paths observed at summary projection time; empty when the workspace cannot provide them. |
| `verify_commands` | Verification command strings found in run evidence. Absence of result evidence never upgrades a command to passed. |
| `exit_code` | Projected process exit code (`0`, `1`, or `130`) for a known terminal status; `null` when unavailable. |
| `pack` | Selected pack `id`, exact `version`, verified exact-byte `hash`, and winning `source` (`extension_root` or `repository`). Omitted when no pack is active. |

Builder Plane and similar callers should gate on both `verdict` and `assurance`,
then use `artifacts_dir`, `acceptance_sheet_path`, and `events_path` as the
machine-readable handoff. On failure they should retain those paths and route
`stop_class` into the existing resume/fix policy rather than treating a nonzero
exit code as permission to weaken acceptance.

For a direct, contract-free `--prompt` run, CommandAgent also bounds a model's
post-write confirmation loop. Consecutive successful reads of files written in
the current run can close the command with exit `0` after the model has already
made the requested changes. This result remains unverified (`static` assurance);
reads without a prior write and reads after an unresolved command failure remain
failures.
