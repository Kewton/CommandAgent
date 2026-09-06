# Issue #429 verification

Date: 2026-09-06. Parent: `0296d779eed87f244899199a5687f8261b449e3f`.
Host: macOS, rustc `1.94.0`, Node `v24.1.0`.

## Final verification

All commands below completed with exit 0 on the final implementation. Focused,
guardrail, corpus, full-target, and doc tests ran outside the sandbox. No new
ignored/skipped tests or modified acceptance gates were introduced.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| `cargo test --lib recovery -- --nocapture` | exit 0; 162 passed |
| `cargo test --test generality_guardrails --test profile_runtime_guardrails` | exit 0; 10 + 2 passed |
| `cargo test --test corpus_regression` | exit 0; 6 passed; also rerun in final all-targets |
| `cargo test --all-targets` | exit 0; 72 result groups, 2,673 passed, 0 failed, 38 existing ignored |
| `cargo test --doc` | exit 0; 2 passed |
| `git diff --check` | exit 0 |

The final all-targets run includes corpus (6 passed), generality (10 passed),
profile-runtime (2 passed), and conformance (18 passed, 1 existing ignored).
The final focused log explicitly includes the real nested-cwd preflight,
first-GET/business-failure preflight, and package-script cwd tests.

Raw local logs are outside the repository at `/private/tmp/issue429-final-`
`focused.log`, `guardrails.log`, `clippy.log`, `all-targets.log`, and `doc.log`
(each suffix follows the shared prefix). They are not committed evidence.

## Scope and observed behavior

- The missing-output fixture starts without `data/`. Its first synthetic GET
  generates exactly `data/staff.json` and `data/shifts.json` in the isolated
  observation. The same check also covers existing empty JSON files.
- The successful probe returns `CurrentSuccess`. The business-failure probe
  returns `Failed` and is fed into the existing Recovery driver, which starts
  attempt 1. Output permission does not manufacture business success.
- Original temporary workspaces retain their source/content hash on successful
  observations, business failures, source/config mutations, unregistered sibling
  additions, and invalid output types. Existing synthetic JSON bytes stay intact.
- Policy and preflight events are compared against the corpus fixture using
  existing field names. The fixture is synthetic evidence, not an S2 recording.
- Negative tests cover repeated lexical `filePath` declarations, unknown/TDZ
  bindings, quoted/commented writer and import lookalikes, an unbound sibling
  named `lib/app/page.ts`, regex/templates (including interpolation), shadowed or
  reassigned fs/path/process/writer identities, computed property replacement,
  and static path prefixes followed by dynamic expressions.
- Exact-file hashing rejects sibling additions and directory masquerading.
  Nested projects, command cwd overrides, and route-bound cwd changes grant no
  root-level output permission; root-layout S2-style writers remain eligible.
  A real nested-cwd GET creates only nested observation data, registers no root
  outputs, and returns `Unavailable` while retaining the original workspace.
  Cwd-changing package scripts (including indirect scripts) disable grants;
  ordinary `next build`/`next dev`/`next start` scripts retain them.
  Source/config paths, config filename variants, imported custom JSON,
  transitive/extensionless config references, JSONC comments/trailing commas,
  and JSON used by non-route `next.config.js`/`.ts` remain protected. Config
  mutations change the protected hash even when a recognized writer names them.
- Absolute/traversing paths and symlink ancestors, leaves, dangling links, and
  observer-created links are rejected. External synthetic sentinels remain
  unchanged. Existing runtime exclusions, including the root `.goal-verify-tools`
  link used by observation setup, remain in place.

## Portability and conservative limits

The executable fixture is plain JS with standard Node ESM/filesystem promises
and a plain status/body object. A separate temporary-fixture probe ran with
`node --no-experimental-strip-types probe.mjs`: success exited 0 and the
business-failure argument exited 1, with both JSON outputs created. It needs no
type stripping, Fetch global, TypeScript loader, or downloaded dependencies.
Older Node binaries were not executed on this host. The typed S2-style writer
remains a static test fixture.

Regex/division and templates deliberately grant no outputs for the containing
source file. Escaped literals, dynamic expressions, mutable/ambiguous bindings,
and other unsupported syntax also grant no corresponding output. See
[design.md](design.md) for the supported grammar and conservative false negatives.
This validation invokes a synthetic GET directly; it is not a live Next.js
browser replay or an S2 business-correctness claim.

## Intermediate failures, kept separate from final verification

1. Initial sandbox `cargo test --lib recovery_ -- --nocapture`: exit 101,
   118 passed / 1 failed. The existing
   `final_acceptance_repair_cycle_reprobes_restart_hook_recovery_to_pass` test
   failed at `src/planner/runner/tests/support/process.rs:27` while reserving a
   loopback port (`PermissionDenied`, `Operation not permitted`). The outside-
   sandbox rerun passed all 119. No product/acceptance change addressed this
   environment restriction.
2. Initial outside-sandbox full run: exit 101 at
   `nextjs_boundary_erosion_tripwire_keeps_dispatch_sites_audited`. The scanner
   counted profile literals in the two new leaf test helpers as production.
   Both files now explicitly wrap their contents in `#[cfg(test)]` modules,
   preserve include/helper visibility, and use the registered profile constant.
   Both generality and profile-runtime guards passed before the final full run.
   No audit exemption or guardrail baseline was added or raised.

## Original session and repository boundaries

Only the original S2 `src/lib/storage.ts` was read to inspect its structure.
No original runtime data was copied or modified, and no original-session
command, HTTP request, or observation was run. Tests use artificial data in
temporary directories. Historical evidence and live `.anvil/` state were not
edited. The #429 include sits beside the existing preflight mutation tests,
separate from #428's imports and #425's ScriptedDriver wiring. No predecessor
was reimplemented or cherry-picked. No push, PR, or service operation was run.
