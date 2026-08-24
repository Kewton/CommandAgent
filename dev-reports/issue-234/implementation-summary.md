# Issues 234 and 235 Implementation Summary

Implemented the combined Epic 260 Lane C change.

## Configuration and role defaults

- Added preset-resolved `planner_think`, `classifier_model`, and
  `classifier_provider` configuration with source attribution and bilingual
  reference documentation.
- Planner calls default to Ollama `think=false`; an explicit global `--think`
  still wins. Step and ultra planners share that role setting.
- Classifier provider/model default to the resolved planner provider/model. A
  different classifier provider requires an explicit classifier model.
- Executor and repair behavior is unchanged: an omitted executor `--think`
  remains omitted.
- Local-provider timeout selection now includes the classifier provider.

## Provider boundary and Gate 1

- Added a classifier-only provider-call override that forces `think=false` and
  caps `num_predict` at 64 without changing shared executor defaults.
- The override builds the configured classifier client/model at the provider
  boundary while preserving injected test clients.
- Gate 1 keeps the existing closed candidate vocabulary, cancellation,
  response-byte limit, timeout, and typed-unknown fallback.
- `provider_turn_duration` now includes additive `think` evidence using the
  existing vocabulary or `omitted` when no Ollama setting was sent.

## Tests and dependency preservation

- Added request-level Ollama tests for planner false, classifier false plus the
  64-token cap, executor omission, and matching event evidence.
- Added configuration tests for defaults, precedence, independent classifier
  selection, and cross-provider validation.
- Added a corpus fixture for the additive provider-turn role evidence and
  updated exhaustive test `Config` literals mechanically.
- Merged Issue 208 commit `4562b134` through merge commit `6a2f5072`. The final
  diff from that dependency in `python_cli.rs` contains only the three new
  mechanical role fields, preserving its package-naming behavior.

## UAT result

Ran one no-retry create scenario per profile with the same executor
(`gemma4:31b-cloud`) and planner (`qwen3.6:27b-coding-nvfp4`) in paired
`planner_think=true` and `planner_think=false` arms. The terminal full-pass band
was equal: think-off `1/4`, think-on `1/4`. Next.js passed in both arms;
python-cli, data, and ingest failed their fixed gates in both arms and were not
promoted. Event audit confirmed `false`/`true` on every applicable planner turn
and `omitted` on executor turns.
