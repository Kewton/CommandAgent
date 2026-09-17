# Issue #490 verification

- Status: `passed`

## Checks

- `cargo test --offline --lib issue490 -- --nocapture`: `passed`
- `cargo test --offline --lib recovery_step_plan_binding`: `passed`
- `cargo test --offline --test corpus_regression --test generality_guardrails`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --offline --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Results and boundaries

The focused suite passed all 8 tests (with per-case loops). Related binding tests
passed 57, including the unchanged
`issue478_admission_and_fallback_refuse_missing_or_nonexecuting_host_duties`,
#466 finite retry/fallback/registration and #488 admission/registration controls.
Corpus passed 7 tests; guardrails passed 10 without baseline changes.
The complete cargo test command exited 0: library tests passed 2,632 with 19
existing ignored tests, followed by successful integration and documentation
tests. Existing opt-in/ignored tests were not enabled. Final clippy has no warnings.

Before the production fix, the normal-flow P02-D test failed with exit 101:
`formation lost original check/expected result or its complete scope and boundary`.
It exhausted the existing three attempts and did not register core. The P01 /
N01–N07 companion test passed on that baseline. This expected red regression is
the before-fix demonstration, not a failed final check. After the fix, identical
P02-D input passes generation, save/reload, before_phase and normal registration,
with no path deletion or Admission intervention.

The runtime test was also run successfully with the original admission ownership
wiring temporarily restored (`cargo test --offline --lib issue490_runtime --
--nocapture`), then with the fix and again in the complete suite. It pins metadata
and precheck applicability for all six proposed steps and executes nine reader
controls: original/retained/additional reader × pass/wrong-port/missing input.
Missing package.json retains both missing-path and existing dependency-boundary
observations; failure and absence enter the ordinary executor/repair boundary
and propagate its controlled error. Neither is counted as a successful precheck.
No runtime verification implementation was changed.

Normal-flow observations assert exact script/path additions, unchanged existing
commands/paths, script-only Implement/pass producer, complete model/host
instructions, unchanged saved/read-back plan and agreement between live and saved
run contracts. README existence stays a plan check, excluded from final-success
commands by the existing artifact-only rule. Reader-attribute and acquired
before/update/after matrices additionally exercise actual Admission capture and
finish/fallback; full-flow controls independently test registration/refusal.
N06 is recorded as host restoration, not evidence of host-loss refusal.

The separate build-loss diagnostic reproduces the frozen sanitized-to-preset
transition, including its original conversion goal, and remains a known issue.
No live application, GUI, model service or #488 projection improvement is claimed.
This is not a release task; release build/version/hash checks are not applicable.

## Execution environment and evidence

Baseline commit: `b13ef093e462084956bb792e10616d6848e00f40`.
Every cargo process exported the dispatched dedicated locations:

```text
CARGO_TARGET_DIR=/Volumes/SSD_NX/tmp/commandagent-orchestrate-490-20260917-01/target
CARGO_HOME=/Volumes/SSD_NX/tmp/commandagent-orchestrate-490-20260917-01/cache/cargo
TMPDIR=/Volumes/SSD_NX/tmp/commandagent-orchestrate-490-20260917-01/tmp
```

Raw logs remain uncommitted in that run's `runtime/`: `pre-fix.log`,
`runtime-baseline.log`, `focused.log`, `related.log`, `corpus-guardrails.log`,
`fmt-check.log`, `clippy.log`, and `full-test.log`. The portable corpus's provenance
hashes and absence of local absolute paths were checked. CI uses only committed
fixtures and temporary workspaces; it does not require these logs or the old SSD
campaign. HOME, live `.anvil`, frozen experiments and external Issue state were
not changed.
