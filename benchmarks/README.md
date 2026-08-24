# Benchmarks

`minimal-loop-expanded.yaml` is the scenario fixture for the expanded minimal
loop benchmark. It names the scenario set and stores each scenario's prompt.
`scripts/bench.sh` uses the selected fixture's `id` values to create one result
row and work directory per scenario and run. The current script records the
benchmark matrix and metadata; it does not execute the prompts in the fixture.

Run the default fixture from the repository root:

```bash
./scripts/bench.sh minimal-loop-expanded --model <model> --runs 3 \
  --max-iterations 12
```

The first positional argument selects `benchmarks/<name>.yaml` and defaults to
`minimal-loop-expanded`. Supported options are:

- `--model <id>` records the model label in each summary row.
- `--runs <count>` records that many rows for every scenario.
- `--max-iterations <count>` records the intended iteration cap in each row's
  `extras_json` metadata.
- `--recheck-root <path>` validates an existing `<path>/summary.tsv` and writes
  `<path>/summary.recheck.tsv`; this mode does not read a scenario fixture.

`--bench-no-debug` is also accepted and records the no-debug setting in
`extras_json`. Normal results are written below `.anvil/benchmarks/` unless
`COMMANDAGENT_BENCH_ROOT` overrides that location.
