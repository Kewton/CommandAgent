# macOS Seatbelt applied-probe record (Issue #655, stage 1a)

Recorded on a non-nested macOS environment where `sandbox-exec -p
'(version 1)(allow default)' true` succeeds (`rc=0`). Drive this record with
the commands below if it needs to be reproduced; the corpus table
`indirect-cases.jsonl` is the machine-checked part and
`tests/issue502_os_sandbox_write.rs` runs it.

## Sandbox profile used

Writes are confined to the workspace (`WS`), the run temp directory (`TMP`), the
known dependency caches (cargo registry/git/package-cache, npm, pip,
Playwright browsers), and device files (`/dev/*`). Reads and network are not
changed.

## Write-escape probes (must fail)

| command form | sandbox outcome | outside fixture |
| --- | --- | --- |
| `sh -c 'cp a.txt escape/secret.txt'` | denied | unchanged |
| `eval 'echo x > escape/secret.txt'` | denied | unchanged |
| `xargs -I{} cp {} escape/secret.txt` | denied | unchanged |
| `python3 -c 'open("escape/secret.txt","w")'` | denied | unchanged |
| `find . -exec cp {} escape/secret.txt \;` | denied | unchanged |
| `dd of=escape/secret.txt` | denied | unchanged |
| `sed -i ... escape/secret.txt` | denied | unchanged |
| `ln escape/secret.txt hard.txt` (outside inode) | denied (EPERM) | unchanged |
| write to the system temp directory | denied | not created |

## Required tools inside the sandbox (must succeed)

| command | outcome |
| --- | --- |
| `mktemp -p "$TMPDIR" probe.XXXXXX` | success |
| `mktemp "$TMPDIR/probe.XXXXXX"` | success |
| `git status --porcelain` | success |
| `cargo --version` | success |
| `node -e '...'` | success |
| `python3 -c '...'` | success |
| a workspace write (`echo inside > inside.txt`) | success |

`mktemp` note: macOS `mktemp` ignores `TMPDIR` and would target the per-user
temp directory, which the sandbox denies by design. Passing `-p "$TMPDIR"` or a
template inside the run temp directory makes it succeed. A bare `mktemp` fails
under the sandbox and is not claimed to work.

## Heavy probes

| probe | outcome | note |
| --- | --- | --- |
| dependency-less `cargo build` | success | writes only inside the workspace `target/`; cargo caches allowed |
| `npm ci` (cached, offline) | success | writes only inside the workspace and the npm cache |
| `next build` (cached Next.js, offline, `NEXT_TELEMETRY_DISABLED=1`) | success | writes `.next/` inside the workspace |
| Playwright Chromium launch (headless, cached browser) | success | `PLAYWRIGHT_BROWSERS_PATH` cache allowed; no nested-sandbox failure observed on this host |

If a future host fails a heavy probe, the cause is recorded here and the
allow-list or `--no-sandbox` need is documented rather than hidden.

## Unavailable-backend probes

Simulated nested sandbox (`rc=71`) is covered by
`tests/issue502_os_sandbox_write.rs`:

- `auto`: warns once and records `bash_sandbox_status` with `applied=false` and
  the reason; Bash and verifier commands continue with lexical checks only.
- `required`: Bash and verifier commands fail with `bash_sandbox_unavailable`.
- `off`: no warning, `applied=false`, no reason; the previous behavior is kept.

A real nested invocation (`sandbox-exec` inside `sandbox-exec`) exits `71`
(`Operation not permitted`), which the probe reports as unavailable.
