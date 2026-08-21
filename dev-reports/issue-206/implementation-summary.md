# Issue 206 implementation summary

## Outcome

Bash calls now fail with the existing recoverable
`bash_path_confinement_error` before execution when a recognized filesystem
mutation would leave the canonical workspace boundary. The shared runtime
policy also records the call as blocked before unattended `--yes` or GUI
delegation can execute it.

## Changes

- Added a leaf Bash write guard that identifies output redirects and destination
  operands for `ln`, `cp`, `mv`, `tee`, `mkdir`, `install`, `chmod`, `chown`,
  `rm`, `touch`, and `truncate`.
- Confined shell working-directory changes, dynamic/home-relative write targets,
  existing symlink components, and new symlink targets. `/dev/null` remains an
  allowed output sink.
- Reused canonical workspace path validation for existing targets and missing
  leaves, so a pre-existing symlink or an in-command symlink escape is rejected
  before `sh -c` starts.
- Wired the result into `runtime_bash_policy` with `blocked: true`,
  `bash_path_confinement_error`, and a reason. Existing destructive-command
  hard-error behavior remains authoritative.
- Extended `bash_path_confinement_rejected` additively with `schema_version`,
  `blocked`, `reason`, and `operation`; the event name and existing fields are
  unchanged.
- Clarified `--yes` help and `SECURITY.md` without claiming that static Bash
  inspection is a kernel-level sandbox.
- Added focused unit and integration coverage and updated the existing Bash
  confinement corpus fixture.

## Compatibility

- Normal workspace-relative commands, redirects, and internal symlinks continue
  to execute.
- System paths remain usable as read operands; write destinations are evaluated
  separately.
- Existing recovery classification and workspace-relative retry guidance are
  preserved.
