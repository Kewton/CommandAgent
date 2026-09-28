# Security Model

`commandagent` is a local-first coding agent for trusted workspaces and trusted
goals. It is designed to reduce accidental damage from model tool calls, not to
execute hostile projects or hostile instructions safely.

## Trust Boundary

- The active workspace is trusted. Do not run the agent on a repository whose
  scripts, package hooks, or source tree you would not run manually.
- The requested goal is trusted. A malicious prompt can still ask the agent to
  run dangerous commands or edit sensitive files.
- Bash commands are policy-checked before execution. Recognized filesystem
  write destinations (common mutating commands and output redirects) must stay
  under the canonical workspace root, including through existing symlinks;
  newly created workspace symlinks may not target paths outside that root.
  Bash is not a complete sandbox: shell commands and invoked programs still run
  as the current OS user, and static inspection cannot prove every indirect
  effect of an arbitrary program.

## `--allow` and `--yes`

`--allow` installs a hard tool-class ceiling for a run. `read` admits
Read/Glob/Grep, `write` admits and auto-approves Write/Edit, and `bash:verify`
admits and auto-approves only Bash commands accepted by the shared verifier
policy; recognized direct filesystem-mutating Bash commands remain excluded.
Repeat the option or comma-separate selectors. Once an explicit list is used,
omitted classes are blocked before execution.

`--yes` is the backward-compatible all-tools alias and also skips resume
confirmation. Neither option bypasses the recognized Bash write-destination
guard. Use `--yes` only in a trusted workspace after checking the goal and
current branch: it does not turn Bash into an OS sandbox and does not auto-kill
unrelated local processes.

At run startup CommandAgent warns about a non-Git workspace or existing Git
changes. At exit it reports the final tracked/staged diff stat and untracked
files. The report describes final workspace state and may include changes that
predated the run.

`--offline` blocks runtime dependency setup plus Bash command families named in
`--help`. It does not block provider/API requests or arbitrary network-capable
programs and therefore is not a network sandbox.

## Child Process Environment

Bounded child processes start from `env_clear` and receive only a small
allowlist such as `PATH`, `HOME`, locale variables, `TERM`, selected Node/npm
settings, and explicit verifier/dev-server variables. Provider API keys and
unrelated parent environment variables must not be inherited by child commands.

## Symlink Policy

Write/Edit reject a symlink as the final target, re-check existing components
under the workspace root, and use no-follow open flags where the platform
supports them. Existing intermediate symlinks are allowed only when their
canonical target remains under the workspace root.

This narrows symlink escape risk but is not a complete kernel-level sandbox.
There remains a TOCTOU window on intermediate path components; hostile
workspaces require a separate containment layer.

## Credential Paths

Read, Glob, Grep, Write, and Edit refuse workspace credentials for every
`WorkspacePolicy` and independently of the metadata exception. `--yes` and
`--allow` do not lift this block; there is deliberately no opt-in to read
workspace credentials in this change.

- Denied names (ASCII case-insensitive, nested components included): `.env`,
  `.env.*`, `.envrc`, `.npmrc`, `.pypirc`, `.netrc`, `.git-credentials`,
  anything under `.ssh`, `.aws/credentials`, `.docker/config.json`,
  `.config/gcloud/application_default_credentials.json`, `service-account*.json`,
  `*.private.key`, `*.private.pem`, `*.p12`, and `*.pfx`.
- The only exceptions are the exact basenames `.env.example`, `.env.sample`, and
  `.env.template`. That exception applies to a template *file* only: a directory
  whose name is a template (for example `.env.example/`) is a protected parent,
  so its contents are denied and are pruned from the bounded broad Bash walk to
  match the shared predicate. `.env.example.local` and
  `.env.production.example` are refused, and a template name that is a symlink
  to a credential is refused through its canonical target. Ordinary `.pem`/`.key`
  files are not denied.
- The canonical target is always checked, not only when the final component is a
  symlink: a credential reached through a symlinked parent directory (for
  example `keys` -> `.ssh`, or `cfg` -> `.aws`) is refused for Read/Write/Edit
  and Grep, whether the request names the alias or the real path. A new target is
  judged by its canonical existing parent plus the created name.
- A path whose canonical target resolves outside the workspace root is refused
  with path confinement and is never followed: an explicit request fails, and a
  broad walk excludes it. A `None` credential verdict is never read as proof of
  containment.
- Broad Read directory listings, Glob, and Grep exclude credentials and
  out-of-root targets instead of leaking their names or contents; an explicitly
  named secret glob, including a compound path such as `.aws/credentials` or a
  symlink alias to a credential, is refused, and the broad exclusion is reported
  as a non-leaking workspace-policy notice so an empty or shorter result is not
  mistaken for "nothing exists".
- Bash refuses an explicit credential path before execution and before a general
  approval request. The bounded broad-grep/broad-find rewrites use the same
  case-insensitive name and compound rule (keeping the strict templates), and the
  caller-supplied command is scanned before that rewrite, so a command appended
  after a recognized broad prefix is still refused.
- Extension tool `WorkspacePath` arguments refuse credentials, and an unmediated
  directory `WorkspacePath` is refused because nested credentials cannot be
  guaranteed before the executor runs.
- Pack input reads and declarative command-check argv refuse credentials at
  registration and before spawn.

Because the refusal happens before any read, a rejected call returns an honest
refusal that contains no credential bytes or expansion value. This is not a
claim that no arbitrary Bash program can read a secret: dynamic variables,
indirect scripts, encodings, hardlinks, copies, and TOCTOU are tracked
separately by #502. Keep real secrets outside the agent workspace and edit the
template files instead.

## Backlog

- Container-sandboxed execution track: run Bash/verifier commands inside a
  per-workspace container or equivalent OS sandbox with a mounted workspace,
  minimal environment, and explicit network policy.
