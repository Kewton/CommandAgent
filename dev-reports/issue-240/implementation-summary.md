# Issue #240 Implementation Summary

## Probe battery

- Upgraded the fixed battery to `model-probe-v3` and added one bounded
  closed-list classifier task using the configured classifier provider/model,
  existing `think=false` override, response bound, and generation cap.
- Added the resolved classifier identity and per-role measurements to the JSON
  report and Markdown card. Executor, planner, and classifier now report task
  pass counts, a fixed-probe completion band, provider-turn count, total
  provider duration, latency statistics, and token telemetry.
- Kept existing aggregate metrics and task/event shapes, adding only
  classifier validity and role measurement fields. The probe band is explicitly
  not a production capability tier.
- Changed generated profile/card basenames to include all three role model IDs,
  preventing different role combinations measured in the same second from
  overwriting each other.

No provider implementation or `src/config.rs` was changed.

## Measured recommendation

Ran two observations each of four local Ollama configurations using the Issue
candidate release build and pinned model digests. The committed record includes
the exact build, host, command shape, model digests, per-role bands/durations,
and SHA-256 checksums for all generated JSON profiles and Markdown cards.

The evidence supports `qwen3.8:27b-mlx` for executor/planner plus
`qwen3.5:4b` for the classifier as a local probe/smoke starting point. The
independent 4B classifier completed 4/4 relevant observations and measured
176–304 ms in the final hybrid. The evidence does not support a smaller
planner: warm 9B was slower than 27B, and 4B planning met the JSON contract in
only 1/2 observations. The docs report that negative result instead of
publishing the proposed but unmeasured 2–3x gain.

## Documentation

- Added matching English and Japanese model-probe guides and kept the legacy
  `docs/guide/model-probe.md` path as a bilingual landing page.
- Added matching role-selection guidance to both provider guides.
- Replaced the uncited local preset example in both configuration guides with
  the measured classifier-only split and linked evidence/remeasurement limits.
- Added a repository-owned bilingual measurement record under
  `docs/guide/model-probe-results/`.

The documented preset is not a built-in default and does not auto-configure a
workspace. The generated config template remains unchanged.

## Runtime cleanup

The eight JSON profiles and eight Markdown cards created by the live Issue
probe were identified by their exact v3 role-qualified names, summarized and
checksummed in repository evidence, then removed. No pre-existing file under
`~/.anvil/model-profiles` was targeted or altered.
