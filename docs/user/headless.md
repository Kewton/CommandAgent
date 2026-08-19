# Headless execution

Use `--summary-json` with a normal execution command when another process needs
the terminal result. CommandAgent keeps its existing human output and writes one
compact JSON object as the final stdout line after run evidence has been closed.
Omitting the flag preserves the existing stdout bytes.

```bash
commandagent --yes --no-footer --summary-json \
  --profile nextjs --ultra-plan-run "Create the app" \
  | tee commandagent.log
jq -R 'fromjson? // empty' commandagent.log | tail -n 1
```

## Exit codes

| Code | Meaning |
| ---: | --- |
| `0` | The command process completed. Read `verdict` and `assurance`; process success alone does not upgrade acceptance. |
| `1` | Execution or validation failed. The summary is still the final stdout line when a run was started; read `stop_class` and the evidence paths. |
| `2` | CLI arguments were rejected before a run started, so no run summary is available. |
| `130` | The process received `SIGINT`. Use the persisted event path shown by the run while it was active. |

## JSON schema

The schema identifier is `commandagent.headless-summary/v1`. Existing scalar
keys are always present; unavailable measurements are JSON `null`. The
additive `pack` object is omitted when no pack was selected.

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
| `stop_class` | Failed terminal event `failure_kind`; `null` for a successful terminal. |
| `directive_round` | Latest persisted directive round, or `0` for an ordinary non-directive run. |
| `pack` | Selected pack `id`, exact `version`, verified exact-byte `hash`, and winning `source` (`extension_root` or `repository`). Omitted when no pack is active. |

Builder Plane and similar callers should gate on both `verdict` and `assurance`,
then use `artifacts_dir`, `acceptance_sheet_path`, and `events_path` as the
machine-readable handoff. On failure they should retain those paths and route
`stop_class` into the existing resume/fix policy rather than treating a nonzero
exit code as permission to weaken acceptance.
