# Issue #230 design: explicit tool allowance and workspace change visibility

## Scope and current behavior

The CLI currently treats `--yes` as one blanket approval bit. Read-only tools
run without approval, while Write, Edit, and Bash all consult the same bit.
The Bash boundary still applies workspace path confinement and dangerous/offline
command checks, but callers cannot grant writes while withholding Bash. CLI
runs also start and stop without showing whether the selected workspace was a
Git worktree or which tracked and untracked changes remain at exit. Finally,
`--offline` is enforced for runtime dependency setup and a bounded list of Bash
network/setup commands, but its exact boundary is not represented in doctor
output.

The approved row decision assigns these policies to `src/tools` leaves and
explicitly excludes GUI delegate changes. No predecessor changes are pending;
this branch starts at the same commit as `origin/develop`.

## Design

1. Add a tool-allow policy leaf with the public selectors `read`, `write`, and
   `bash:verify`. `--allow` accepts repeated or comma-delimited selectors. An
   explicit list is a hard ceiling: read covers Read/Glob/Grep, write covers
   Write/Edit, and bash:verify admits Bash only when the existing shared verify
   command policy accepts the command. Selected mutating classes are
   auto-approved. With no explicit list, existing approval behavior remains;
   `--yes` remains the all-tools alias and still preserves workspace confinement,
   dangerous-command checks, and resume confirmation semantics.
2. Install the parsed policy for the scoped CLI run and enforce it at the tool
   execution boundary. Also check the normalized split-Bash path, which can
   execute outside the ordinary registry dispatch. A denial is returned as a
   tool error before a child process or filesystem mutation starts.
3. Add a Git-state leaf that uses bounded, read-only `git` subprocesses. Before
   a run it warns when the workspace is not Git-managed, cannot be inspected,
   or already has tracked/untracked changes. At scope exit it renders the final
   tracked diff stat (including staged changes when a HEAD exists) and a
   separate untracked-file list. The report is explicitly workspace state at
   exit, so it does not falsely attribute pre-existing changes to the agent.
4. Add an offline-scope leaf as the single source for CLI help and doctor
   wording/details. It will state the enforced Bash/runtime setup categories
   and explicitly state that provider/API requests and other network-capable
   commands are unaffected.

## Compatibility and safety

- Existing invocations without `--allow` retain read access and the existing
  mutation-approval behavior. `--yes` remains backward compatible as full
  auto-approval, not a bypass of workspace or Bash safety checks.
- Git inspection is read-only, bounded, and best effort. A failed inspection
  warns rather than aborting the requested run. Reports go to stderr so stdout
  and `--summary-json` contracts remain unchanged.
- No GUI delegate, runner chokepoint, event schema, corpus contract, historical
  evidence, or `.anvil/` namespace changes are included.

## Verification

Run focused CLI/tool policy, Git-state, doctor, and Issue integration tests
first. Because this changes shared CLI and tool execution behavior, then run
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
the full `cargo test` suite.
