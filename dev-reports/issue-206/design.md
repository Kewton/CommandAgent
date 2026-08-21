# Issue 206 design

## Problem

The Bash tool applies a lexical absolute-path check before launching `sh -c`,
but that check allows common system prefixes so commands can still target
locations such as `/usr/local/bin`. In unattended `--yes` and GUI-delegated
runs this contradicts Gate 1's workspace-only change boundary. The existing
`bash_path_confinement_rejected` event also does not explicitly record the
blocked decision or its reason.

## Smallest coherent change

- Add a leaf Bash write-target guard that tokenizes simple shell segments and
  identifies output redirections plus destination operands for the Issue's
  write-capable command families (`ln`, `cp`, `mv`, `tee`, `mkdir`, `install`,
  `chmod`, `chown`, `rm`, `touch`, and `truncate`). Shell working-directory
  changes are validated as well so a later relative destination cannot escape
  by first running `cd`.
- Validate each identified destination against the canonical workspace root,
  including existing symlink components, newly created symlink targets, and
  missing leaves. `/dev/null` remains an allowed output sink; unresolved
  dynamic or home-relative write targets fail closed.
- Run this check before the existing general absolute-path allowance. Return
  the established `bash_path_confinement_error` so current recovery behavior
  and schema names remain compatible.
- Extend `bash_path_confinement_rejected` with `blocked: true`, `reason`, and
  `operation` fields. Existing fields remain unchanged.
- Clarify the `--yes` help and security documentation: approval is skipped,
  but recognized Bash write targets remain workspace-confined and Bash is not
  a complete OS sandbox.

## Tests

- Unit-test tokenization/write-target classification and canonical path checks.
- Add an integration test through `ToolRegistry` proving an outside symlink
  destination and outside redirection are not created, their events contain
  `blocked: true` and a reason, and normal in-workspace Bash writes/symlinks
  still succeed.
- Update the Bash confinement corpus fixture for the additive event contract.

## Non-goals

- No event rename or schema migration.
- No kernel/container sandbox is introduced in this Issue; arbitrary programs
  can still perform effects not visible to static shell-token inspection.
- No Gate 1 identity or GUI delegation flow changes are needed because both
  paths already execute through the shared tool registry.
