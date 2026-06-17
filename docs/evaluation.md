# Evaluation

CommandAgent evaluation treats verifier output as first-class triage data.
Failures should show what command ran, why it failed, and the smallest useful
evidence packet for repair.

Eval case YAML lives under `eval/cases`. Case sets are split into `smoke`,
`small`, and `large`. Large cases should use semantic checks based on required
artifacts, verifier commands, and content signals rather than line count alone.

`scripts/eval_agent_slice.sh` runs a case directory with the release binary and
writes a timestamped root containing per-run `meta.json`, stdout/stderr, a
workspace directory, and `summary.tsv`. Use `--dry-run` for offline wiring
checks. The runner records `success_check` in `meta.json` and applies semantic
checks for required paths and required file content signals in addition to the
process return code and expected artifacts.

`scripts/eval_report.py <root>` summarizes `summary.tsv` by headline success,
failure category, and case. `scripts/eval_report.py <root> --recheck` rechecks
existing workspaces against current case `success_check.required_paths` and
`success_check.must_include`, then writes `recheck_summary.tsv` without
overwriting the original summary.

The eval runner executes cases through the mode declared in each case. Omitted
mode defaults to `/plan-run`; large cases should normally use `/ultra-plan-run`.
Modification cases can declare a fixture directory, which is copied into each
run workspace before execution.

Case `intent` is passed to the slash command as `--intent`. Case
`expected_artifacts` are passed as repeated `--artifact` flags and are also
checked after the run. This keeps the runtime task contract and the success
check contract aligned; expected artifacts are not only post-hoc eval checks.

Large task eval uses:

```bash
scripts/eval_large_tasks.sh
```

The default is `runs=1` for MVP sign-off because each large case can be slow.
Use release-quality mode when comparing stability:

```bash
scripts/eval_large_tasks.sh --release-quality
```

This runs each large case 3 times.

## Verifier Failure Shape

Each verifier failure records:

- `command`: the local command that was attempted
- `reason`: `command_failed:<status>`, `dependency_missing`, or `blocked:<class>`
- `stdout_excerpt` / `stderr_excerpt`: bounded raw output
- `diagnostic_excerpt`: lines likely to matter for repair, such as type errors
  or failed compile messages
- `source_excerpt`: when output references a source location, nearby source
  lines are included with the failing line marked

`dependency_missing` means the verifier could not run honestly because required
local dependencies are absent. For example, `npm run build` with a Next.js build
script requires `node_modules/.bin/next`. CommandAgent must not rewrite build
scripts to fake success; it should install dependencies only when an explicit
setup step and the evaluation environment allow it, or stop with the explicit
dependency-missing reason.

Treat `dependency_missing` as a cross-profile environment/setup boundary, not as
a generic implementation failure. Next.js may report missing `node_modules`,
Python/FastAPI may report missing virtualenv packages, and data tasks may report
missing local tooling. Eval reports should keep this category separate so a run
does not look like a code-quality failure when the verifier was unavailable.

Verifier evidence is deterministic context for the next repair or replanning
step. It is not a semantic sidecar summary.

## Recent Recovery Check

The R5/R6 guard subset at
`eval/runs/r5-r6-guard-subset/20260617T213505` was run from clean commit
`8eff913`.

Result:

```text
large-nextjs-app-modify  false  dependency_missing
large-rust-app-modify    false  rc:1
large-rust-app-new       true   ok
```

The key interpretation is that Next.js remains an environment/setup boundary,
Rust modify moved past the prior missing-artifact/no-tool class and now fails on
compile/edit-repair quality, and Rust new passed the current artifact/process
contract. Details are in
`docs/eval/triage/post-8eff913-r5-r6-guard-subset-20260617T213505.md`.

The stale Edit-target evidence check at
`eval/runs/stale-edit-target-rust-modify/20260617T230748` was run from clean
commit `b68b9ed`. The code now classifies stale Edit failures as
`edit_target_not_found`, but that run did not reproduce the stale Edit class.
It failed earlier on repeated no-tool responses while `src/commands.rs` was
still missing. Details are in
`docs/eval/triage/post-b68b9ed-stale-edit-rust-modify-20260617T230748.md`.

The missing expected path step-contract check at
`eval/runs/rust-modify-missing-path-contract/20260617T235542` and
`eval/runs/rust-new-missing-path-contract/20260618T000711` was run from clean
commit `ac4e833`. Rust modify moved past the targeted missing-artifact/no-tool
class and reached a later Rust module compile error. Rust new failed in a
different class: compile error plus stale Edit repair. Details are in
`docs/eval/triage/post-ac4e833-rust-missing-path-contract-20260617T235542.md`.

The R6 repair focus check at
`eval/runs/r6-repair-file-fix-contract-rust-subset/20260618T004917` was run
from clean commit `6f2df38`. Rust new passed the focused smoke. Rust modify
still failed, but moved beyond the original missing-artifact/no-tool class and
now looks like implementation-quality / phase-decomposition residue. Details
are in
`docs/eval/triage/post-6f2df38-r6-repair-focus-rust-subset-20260618T004917.md`.

## Repair Exhaustion

Bounded repair should stop after the configured file-changing attempt budget.
The exhaustion report records missing expected paths, repeated changed files,
and verifier evidence. For explicit replanning, CommandAgent saves a short
repair packet under `.commandagent/repairs` and suggests:

```text
/ultra-plan-run --profile <profile> "$(cat .commandagent/repairs/<file>.md)"
```

The saved packet is intentionally bounded so it can be fed back through the
slash command parser without turning the whole failed session into a new goal.
