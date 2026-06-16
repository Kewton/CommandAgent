# Evaluation

CommandAgent evaluation treats verifier output as first-class triage data.
Failures should show what command ran, why it failed, and the smallest useful
evidence packet for repair.

Eval case YAML lives under `eval/cases`. Case sets are split into `smoke`,
`small`, and `large`. Large cases should use semantic checks based on required
artifacts, verifier commands, and content signals rather than line count alone.

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
scripts to fake success; it should install dependencies when allowed, or stop
with the explicit dependency-missing reason.

Verifier evidence is deterministic context for the next repair or replanning
step. It is not a semantic sidecar summary.

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
