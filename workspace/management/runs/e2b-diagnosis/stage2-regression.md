# E-2b stage 2 regression bisection

The requested test name `compile_error_repair_prompt_and_reprobe_passes` is
not present in the current test registry (`cargo test --lib -- --list`).  The
corresponding existing test is
`planner::runner::tests::compile_error_repair_prompt_anchors_file_and_then_runs_readiness`.

## Candidate bisection

- `583c1a4` (fix schema wiring): all three attempts failed at compilation,
  before the test ran. The failure was `E0425: cannot find function load_fix in
  this scope` at `src/planner/intent_schema.rs:87:15`; the test module called
  `load_fix()` without importing it.
- `a588bb8` (schema role/import completion): the corresponding test compiled
  and failed at `src/planner/runner.rs:16960:9` in the repaired-report
  assertion. The observed report had an initial compile error
  `src/components/SpaceInvaders.tsx:137:28: Type error: Cannot find name
  'reset'.` and ended with `browser_readiness_failed:start_exited`, rather
  than the expected successful readiness evidence.
- Baseline `22512a9`: the corresponding test passed 3/3.

The candidate split is therefore not a clean runtime attribution: 583c1a4 is
not runnable because its own test helper is incomplete, while a588bb8 exposes
the runtime assertion failure. The requested proof cannot be promoted.

## Call-path reading

The failing test constructs a `nextjs` `UltraPlan` directly, calls
`ultra_final_acceptance_report`, builds a repair prompt, and invokes
`run_final_acceptance_repair_with_ultra_session`. It does not call
`fix_plan_synthesis::phase_plan`, `ensure_contract_shape`, or
`intent_schema::load_fix`. The stage-2 files are therefore not on this test's
runtime call path. The observed difference must be reviewed separately; no
schema proof is claimed.

Raw outputs: `/tmp/e2b-583-1.txt`..`3.txt`, `/tmp/e2b-head-1.txt`..`3.txt`,
and `/tmp/e2b-base-1.txt`..`3.txt`.

## A-B-A port-probed matrix

The test allocates ports dynamically from `TEST_DEV_SERVER_PORT`, starting at
34011 (`runner.rs:18048`); the package fixture itself uses Next's 3011 only
for its generated scripts. Port 34011 was probed immediately before each
run. Sandbox bind probes reported permission denied for the first HEAD pair;
the escalated probes then reported `EADDRINUSE` for the remaining runs,
showing that the port was not free at probe time.

| position | runs | probe | result |
|---|---:|---|---|
| HEAD `227da6e` (initial) | 2 | sandbox probe denied | fail, `browser_readiness_failed:start_exited` |
| baseline `22512a9` | 1 | free | pass |
| baseline `22512a9` | 1 | `EADDRINUSE` | pass |
| HEAD `227da6e` (return) | 2 | `EADDRINUSE` | pass, 1 test each |

Because HEAD passed after the environment changed, the earlier 3/3 failure
was environmentally confounded rather than a stable schema regression. The
stage-2 proof may proceed to the full-suite run; no proof claim is made until
that run completes.
