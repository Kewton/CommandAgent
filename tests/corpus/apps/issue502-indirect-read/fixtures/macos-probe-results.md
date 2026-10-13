# macOS Seatbelt applied read-probe record (Issue #657, stage 2)

Recorded on a non-nested macOS environment where `sandbox-exec -p
'(version 1)(allow default)' true` succeeds (`rc=0`). Drive this record with
the commands below if it needs to be reproduced; the corpus table
`indirect-cases.jsonl` is the machine-checked part and
`tests/issue502_os_sandbox_read.rs` runs it.

## Read layer used

The fixed write profile gains a read-deny layer: each home secret location
(`~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.netrc`, `~/.git-credentials`, `~/.npmrc`,
`~/.pypirc`, `~/.docker/config.json`, `~/.kube`, `~/.config/gh`,
`~/.config/gcloud`) is passed as a `-D` parameter and denied with `(deny
file-read* (subpath …))`; a generated regex denies the workspace credential
names and a second, later regex re-allows the three strict dotenv templates.
Reads are otherwise unchanged (`(allow default)`), so the toolchain, git, and
the system stay readable.

## Read-escape probes (must fail)

| command form | sandbox outcome | home key canary |
| --- | --- | --- |
| `cat "$HOME/.ssh/id_ed25519"` | denied | not leaked |
| `cat "$HOME/notes.txt"` (symlink to the key) | denied | not leaked |
| `cat "$BASH_ENV"` (`BASH_ENV` = the key) | denied | not leaked |
| `python3 - <<'PY' …` (assembles the name) | denied | not leaked |
| `cat <workspace>/link.txt` (symlink to `.env`) | denied | workspace `.env` canary not leaked |
| `cat <workspace>/.env` | denied | workspace `.env` canary not leaked |

## Required tools inside the sandbox (must succeed)

| command | outcome |
| --- | --- |
| `cat <workspace>/.env.example` | success (template) |
| `git status --porcelain` | success |
| `cargo --version` | success |
| `node -e '…'` | success |
| `python3 -c '…'` | success |
| a workspace read (`cat a.txt`) | success |
| a non-credential home read (`~/.config/other/settings`) | success |

## Linux (Issue #657, stage 2)

The Landlock read layer denies the same home secret list by rewriting it as a
read-allow list, and keeps the toolchain, git, and the system readable and
executable (recorded by `linux_applied_toolchain_git_and_system_are_readable_and_executable`).
A workspace credential is closed lexically for a named reference; a non-secret
symlink to it is not closed by the Linux read layer (decision 4) and is recorded
as a documented gap by
`linux_workspace_credential_symlink_is_not_closed_by_read_isolation`. Denying the
home npm config means an `npm` that reads a private-registry `~/.npmrc` fails
inside Bash (the consequence of not feeding registry tokens to Bash).

## Unavailable-backend probes

A simulated nested sandbox (`rc=71`) is covered by
`tests/issue502_os_sandbox_read.rs`: `required` fails honestly with
`bash_sandbox_unavailable` and `auto` records a reason and continues with lexical
checks only, so `bash_sandbox_status` states the read isolation through its
`reason` (a named clause while applied, or an empty reason for a fully confined
sandbox) rather than a new field.
