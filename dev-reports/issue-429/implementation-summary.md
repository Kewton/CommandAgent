# Issue #429 implementation summary

Recovery preflight now registers statically verified JSON output paths before
the files exist. This supports lazy initialization of `data/staff.json` and
`data/shifts.json` in the root-layout S2 writer shape without treating those
outputs alone as protected-source mutations.

Implementation: `61b7b1c4`. See the [design](design.md) for recognition rules and
the [verification record](verification.md) for executed checks, results, and
intermediate failures.

## Changed modules

- [recovery_observation_policy.rs](../../src/planner/recovery_observation_policy.rs)
  combines recognized outputs with contract and configuration exclusions.
- [nextjs_outputs.rs](../../src/planner/recovery_observation_policy/nextjs_outputs.rs)
  resolves route-bound Node writers, lexical constants, exact paths, and cwd
  eligibility; [syntax.rs](../../src/planner/recovery_observation_policy/nextjs_outputs/syntax.rs)
  provides the constrained tokenizer and binding checks.
- [nextjs_configs.rs](../../src/planner/recovery_observation_policy/nextjs_configs.rs)
  excludes configuration variants and referenced JSON, including JSONC and
  non-route `next.config.js`/`.ts` references.
- [recovery_snapshot.rs](../../src/planner/recovery_snapshot.rs) excludes exact
  registered files from the observation hash and rejects directory masquerading
  and non-runtime symlinks.
- [policy tests](../../src/planner/recovery_observation_policy/nextjs_outputs/tests.rs)
  and [preflight tests](../../src/planner/recovery_json_preflight_tests.rs) use
  explicit test-only modules and a [synthetic corpus](../../tests/corpus/apps/issue429-lazy-json-preflight/README.md).
  `auto_recovery.rs` adds only the test include, separate from #425/#428 wiring.

## Policy and limits

Registration grants normalized exact JSON files, never a `data/` directory or
filename-prefix exclusion. Required/protected paths, source/configuration files,
internal namespaces, absolute/traversing paths, symlink components, and existing
directories remain ineligible. Existing runtime exclusions remain unchanged.

Only workspace-root App Router layouts are supported. Nested layouts, cwd
selectors in registered commands or package scripts, and route-bound `chdir`
references disable grants conservatively. All package scripts are checked to
cover indirect calls; an unrelated cwd-changing script can also suppress grants.
The writer's directory is never used to infer cwd. Ordinary root-layout
`next build`/`next dev`/`next start` scripts remain supported.

This is not a general JS/TS parser. Regex/division and templates grant no outputs
for their source file. Escaped literals, dynamic path expressions, unsupported
imports/initializers, and mutable, shadowed, reassigned, or unresolved bindings
grant no corresponding output. Conservative false negatives retain the failure
gate. Existing event fields and acceptance decisions are preserved: business
failures remain `Failed`; protected mutations remain `Unavailable`.

## Evidence limits

Tests use artificial data and isolated temporary workspaces. Plain-JS synthetic
GET handlers exercise preflight without TypeScript stripping or Fetch globals;
typed storage remains a separate static fixture. Event fixtures are synthetic,
not S2 recordings. The original S2 source was inspected read-only; its runtime
data was neither copied nor changed. No live Next.js/S2 UAT or browser replay is
claimed.

The linked verification records passing focused, corpus, guardrail, full-target,
formatting, lint, and doc checks. This summary is a documentation-only addition;
unchanged code tests were not rerun and no remote mutation was performed.
