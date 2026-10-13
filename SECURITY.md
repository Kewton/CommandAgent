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
  Lexical inspection cannot prove every indirect effect of an arbitrary
  program, so when an OS sandbox is available it also confines the writes and
  the home-credential reads performed inside a command or script (see
  [OS Write and Read Sandbox](#os-write-and-read-sandbox)). Bash is not a
  complete sandbox: shell commands and invoked programs still run as the current
  OS user, network is not confined in this stage, read confinement covers the
  home secret locations rather than every workspace credential on every
  platform, and the sandbox is absent when the backend is unavailable.

## `--allow` and `--yes`

`--allow` installs a hard tool-class ceiling for a run. `read` admits
Read/Glob/Grep, `write` admits and auto-approves Write/Edit, and `bash:verify`
admits and auto-approves only Bash commands accepted by the shared verifier
policy; recognized direct filesystem-mutating Bash commands remain excluded.
Repeat the option or comma-separate selectors. Once an explicit list is used,
omitted classes are blocked before execution. `bash:verify` admits a command by
its classification only: the contents of any script the command invokes are
constrained only by the OS write sandbox, when it is applied, and not by the
classification.

`--yes` is the backward-compatible all-tools alias and also skips resume
confirmation. Neither option bypasses the recognized Bash write-destination
guard or the OS write sandbox. Use `--yes` only in a trusted workspace after
checking the goal and current branch: it does not turn Bash into a complete OS
sandbox and does not auto-kill unrelated local processes.

At run startup CommandAgent warns about a non-Git workspace or existing Git
changes. At exit it reports the final tracked/staged diff stat and untracked
files. The report describes final workspace state and may include changes that
predated the run.

`--offline` blocks runtime dependency setup plus Bash command families named in
`--help`. It does not block provider/API requests or arbitrary network-capable
programs and therefore is not a network sandbox. Network isolation is a later
stage of #502 and is independent of the write sandbox below.

## OS Write and Read Sandbox

`--sandbox auto|required|off` (default `auto`) adds an OS-level write sandbox and
a home-credential read sandbox at the two `sh -c` launch points: the model Bash
tool (the `bash:verify` interval reuses the same function) and the
verification/acceptance commands. The sandbox is defense in depth; the lexical
checks still run first and can refuse a command before the shell starts.

- What the OS closes (when the backend is available): a write performed inside a
  command or script that the lexical guard cannot see — `sh -c 'cp ...'`,
  `eval`, `xargs`, `find -exec`, an interpreter's `open(..., 'w')`, `dd of=`,
  `sed -i` — and creating a hardlink to an inode outside the workspace. The only
  writable locations are the workspace, the run's temp directory, the known
  dependency caches, and device files such as `/dev/null`.
- Read confinement (Issue #657) denies reading the home secret locations
  (`.ssh`, `.aws`, `.gnupg`, `.netrc`, `.git-credentials`, `.npmrc`, `.pypirc`,
  `.docker/config.json`, `.kube`, `.config/gh`, `.config/gcloud`) and, on macOS,
  a workspace credential named by the #501 rule (`.env`, `.env.*`, `.envrc`,
  `.npmrc`, `.pypirc`, `.netrc`, `.git-credentials`, `.ssh`, `service-account*.json`,
  `*.private.key`, `*.private.pem`, `*.p12`, `*.pfx`); the three strict dotenv
  templates (`.env.example`, `.env.sample`, `.env.template`) stay readable. The
  list lives in one leaf module (`src/tools/os_sandbox/secret_locations.rs`) that
  both backends import. This closes the reads the lexical guard cannot see: a
  credential through a non-secret symlink, a file named by an environment
  variable such as `BASH_ENV`, and a name an interpreter assembles at run time.
  On Linux only the home secret locations are denied: a workspace credential is
  closed lexically for a named reference, and a non-secret symlink to it is not
  closed by the read layer (decision 4 of #502). Linux rewrites the deny list as
  a read-allow list built once at run start, so listing a directory the run did
  not allow fails by design: `ls /`, `ls $HOME`, and `echo $HOME/*` do not work
  (the home's siblings are allowed, but `/` and `$HOME` themselves are not
  listed). When a writable location (for example the workspace) is the home
  itself or an ancestor of a home secret, the write allow list already grants
  read on that secret's subtree, so the read layer cannot deny it; the status
  then records that the read isolation is not effective instead of claiming it is
  applied. An empty `HOME` is reported the same way.
- What remains lexical-only: network access, and every command run while the
  backend is unavailable. `--allow bash:verify` and `--offline` are lexical
  policies, so a command they admit can still start a script; only the sandbox
  (when applied) constrains that script's writes and home-secret reads.
  - Runtime dependency setup runs outside the sandbox: the setup steps that
    execute `setup.py`, npm `postinstall`/`prepare` hooks, and similar
    install-time scripts run before or outside the confined `sh -c` launch
    points, so the sandbox does not constrain their writes. Confining dependency
    setup is a later stage (#658).
  - A hardlink that already exists in the workspace before the run and points at
    an inode outside the workspace can still be written through. Seatbelt decides
    by path, so it allows writing the workspace path even though the inode lives
    outside; the sandbox only denies *creating* a new hardlink to an outside
    inode. On Linux that creation needs Landlock ABI2 (kernel 5.19); on an older
    kernel the link is not closed and is named in the `bash_sandbox_status`
    reason.
- Backend availability is probed once at run start. On macOS the backend is
  Seatbelt through `sandbox-exec`; under a nested sandbox it reports `rc=71` and
  is unavailable. On Linux the backend is Landlock; it requires ABI1 (kernel
  5.13) and is unavailable below that, when Landlock is disabled, or when a
  seccomp filter denies the ruleset syscalls. Other platforms have no backend in
  this stage.
- `auto` continues with lexical checks only when the backend is unavailable,
  warns once at startup, and records `bash_sandbox_status` with `applied=false`
  and a reason. `required` instead fails Bash and verifier commands with
  `bash_sandbox_unavailable`. `off` keeps the previous behavior. None of them
  reports "sandboxed" when the sandbox was not applied. The read isolation is
  reported through the same event's `reason`: it names the applied read
  isolation, or is empty for a fully confined sandbox (reads included).
- The dependency caches (cargo registry/git/package-cache, npm, pip, Playwright
  browsers) stay writable so builds and tests work; they remain a contamination
  path outside the workspace. On macOS they live under `~/Library/Caches` and
  `~/.cargo`; on Linux under `~/.cargo`, `~/.npm`, and `~/.cache`. pnpm and yarn
  stores are not allowed in this stage. The home npm config (`~/.npmrc`) is a
  read-denied secret, so an `npm` that reads a private-registry `~/.npmrc` fails
  inside Bash: that is the intended consequence of not feeding registry tokens to
  Bash, not a regression.

macOS `mktemp` ignores `TMPDIR` and targets the per-user temp directory, which
the sandbox denies. A `mktemp` inside the run temp directory (`mktemp -p
"$TMPDIR"` or a template under `$TMPDIR`) succeeds; a bare `mktemp` does not.
The applied write-probe record is under
`tests/corpus/apps/issue502-indirect-write/fixtures/macos-probe-results.md`; the
applied read-probe record is under
`tests/corpus/apps/issue502-indirect-read/fixtures/macos-probe-results.md`.

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
refusal that contains no credential bytes or expansion value. This lexical layer
is not a claim that no arbitrary Bash program can read a secret: dynamic
variables, indirect scripts, encodings, hardlinks, copies, and TOCTOU are why the
OS read sandbox above also denies the home secret locations (and, on macOS,
workspace credentials) at the kernel; the remaining forms are tracked by #502.
Keep real secrets outside the agent workspace and edit the template files
instead.

## Backlog

- OS sandbox, remaining stage of #502: network and dependency-install isolation.
  Stage 1a (macOS Seatbelt write confinement and the shared leaf module), stage
  1b (Linux Landlock write confinement), and stage 2 (home-credential read
  confinement on both backends, Issue #657) are implemented here; the same leaf
  module is the seam the later stage extends.
- Container-sandboxed execution track: run Bash/verifier commands inside a
  per-workspace container or equivalent OS sandbox with a mounted workspace,
  minimal environment, and explicit network policy. Superseded in part by the
  #502 staging above; kept as the alternative with a fully mounted root.
