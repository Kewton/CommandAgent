# Dangerous commands: inventory, defenses, and detection limits

This document is the human list behind
`tests/issue673_dangerous_command_inventory.rs`. The test greps the tree and
fails when a signal call or a recursive delete appears that is not declared in
its `DECLARED` table, or when a declared count changes. The list below explains
each site and, for the sites that are deliberately not defended, why.

This is **detection for review, not isolation**. A green test means "no
undeclared site of the covered kinds appeared", not "no dangerous command can
run". The isolated-container verification of Issue #673 対処の方向 4 is what
bounds execution; that runs outside this repository's tests.

Two incidents motivate this file:

- 2026-10-14 (#673): a `cargo mutants` run replaced
  `signalable_process_group -> Option<i32>` with `Some(1)`; the mutated test
  binary executed `kill(-1, SIGTERM)` and terminated every process the user
  owned. `.cargo/mutants.toml` excludes that selector and `group_is_signalable`.
- The same shape exists at every negative `kill`: a selector that decides the
  target is separated from the `kill` by a function boundary, so a mutant can
  cross the boundary with a dangerous value.

## 1. Signal sites

Column `Defense added` is the non-mutatable `assert!` written **directly in the
function that calls `kill`** (never in a wrapper helper: cargo-mutants replaces
the body of a `()`-returning helper with `()`, deleting a helper's check). The
existing `Err`/`bail!` paths are unchanged; the `assert!` is the last fortress
and never fires for a real value.

| File | Function | Kind | Value source | Existing defense | Defense added |
| --- | --- | --- | --- | --- | --- |
| `src/bounded_process.rs` | `terminate_process_group_by_pid` | `kill(-pgid)` ×2 | `signalable_process_group(child.id())` | `signalable_process_group` (`pgid > 1`) + `Err` path | `assert!(-pgid < -1)` before each `kill` |
| `src/bounded_process.rs` | `terminate_process_group_with_grace` | `kill(-pgid)` ×2 | `signalable_process_group(child.id())` | `signalable_process_group` | `assert!(-pgid < -1)` before each `kill` |
| `src/bounded_process.rs` | `dangerous_sentinel_same_uid_other_group_survives` (`group_exists`) | signal 0 probe | spawned target pid | none | **none** — container-only sentinel (`#[ignore]`, `COMMANDAGENT_ISSUE673_CONTAINER=1`) |
| `src/bin/gui_server/trial_process.rs` | `send_group_signal` | `kill(-process_group)` | `ProcessIdentity.process_group` from `Child::id()` | `group_is_signalable` + `bail!` | `assert!(-process_group < -1)` |
| `src/bin/gui_server/trial_process.rs` | `process_group_exists` | signal 0 probe | `ProcessIdentity.process_group` | `group_is_signalable` | `assert!(-process_group < -1)` |
| `src/planner/runner/acceptance.rs` | `signal_dev_server_process_group` | `kill(-pid)` | dev-server `Child::id()` | none | `assert!(-pid < -1)` |
| `src/planner/runner/acceptance.rs` | `dev_server_process_group_exists` | signal 0 probe | dev-server `Child::id()` | none | `assert!(-pid < -1)` |
| `src/preflight.rs` | `SystemPortControl::kill` | positive pid | `lsof` owner pid | `Option` `Some` + `context` | `assert!(pid > 1)` and `assert!(pid != own pid)` |
| `src/planner/runner/tests/support/process.rs` | `process_group_exists` | signal 0 probe (test support) | spawned child pid | none | **none** — test-only helper, outside this Issue's file scope (see §4) |
| `tests/headless_summary.rs` | inline | positive pid | spawned `Child::id()` | none | **none** — test-only, outside this Issue's file scope |
| `tests/gui_server.rs` | inline | signal 0 probe | spawned child pid | none | **none** — test-only, outside this Issue's file scope |
| `tests/gui_read_only_guard.rs` | inline | none (string literal) | — | — | **none** — asserts the source text of `trial_process.rs` |

`starts_with`-style work is not applicable here: the check is on the value that
is passed to `kill` (`-pgid < -1`), which is exact.

### Selectors listed in `.cargo/mutants.toml`

`signalable_process_group` and `group_is_signalable` return a value, so a mutant
can replace the whole body (`Some(1)` / `true`). They are excluded from
mutation, and removing the exclusion requires the isolated-container evidence of
対処の方向 4. The `assert!` in each `kill` caller is what makes the value safe
even before that evidence exists.

### Functions to compare against `cargo mutants --list`

Selectors and the functions that call `kill`, plus the functions that decide the
value (leader enumerates the list in the isolated container; the worker does not
run `cargo mutants`):

- `signalable_process_group`, `group_is_signalable`
- `terminate_process_group_by_pid`, `terminate_process_group_with_grace`,
  `wait_with_timeout_cancel_force_grace_and_clock`,
  `wait_with_timeout_cancel_and_force`, `wait_with_timeout_and_cancel`,
  `wait_with_timeout`, `run_with_timeout_cancel_and_force`, `run_with_timeout`,
  `spawn_child`, `configure_child_session`
- `send_group_signal`, `process_group_exists`, `terminate_unregistered`,
  `request_stop`, `start_force_watchdog`, `ProcessIdentity`, `register`
- `signal_dev_server_process_group`, `dev_server_process_group_exists`,
  `cleanup_dev_server_child_unix`, `wait_for_dev_server_process_group_exit`
- `SystemPortControl::kill`, `handle_busy_port`, `run_for_goal_with`,
  `parse_lsof_owner`

## 2. Recursive deletes (`remove_dir_all`)

Nine sites under `src` plus seven test helpers. Each is classified by whether a
mutation that moves the target could delete something the run does not own. The
guard is `crate::tools::dir_fd::is_strict_descendant(owned_root, target)`: it
`canonicalize`s an `owned_root` that is derived **independently of the target**
and the target, and requires `target != root && target.starts_with(root)`. That
rejects the root itself, an ancestor, a sibling, a path outside the root, a
`..` that escapes, and an outside-pointing symlink — none of which `starts_with`
alone rejects. The guard is the value that is then removed.

| File | Site | Kind | Owned root | Guard |
| --- | --- | --- | --- | --- |
| `src/bin/gui_server/session_paths.rs` | `rollback_unstarted` | product cleanup | the `runs` dir (`SessionPaths.runs_root`) | added |
| `src/model_probe.rs` | `ScratchWorkspace::cleanup` | product cleanup | `std::env::temp_dir()` (`ScratchWorkspace.root`) | added |
| `src/planner/pack/supply.rs` | `install` (failure path) | product staging | `packs/` (`packs_root`) | added |
| `src/planner/pack/supply.rs` | `install` (success path) | product staging | `packs/` | added |
| `src/tools/os_sandbox.rs` | `cleanup` | product temp | `<state>/sandbox` (`Installed.run_tmp_root`) | added |
| `src/tools/dir_fd.rs` | test swap seam (×2) | test helper | a synthetic tempdir | **none** — synthetic path |
| `src/tools/edit.rs` | test swap seam | test helper | a synthetic tempdir | **none** — synthetic path |
| `src/tools/write.rs` | test swap seam | test helper | a synthetic tempdir | **none** — synthetic path |
| `tests/gui_server.rs` (×4), `tests/issue505_write_toctou.rs` (×3) | test cleanup | test helper | synthetic tempdirs | **none** — synthetic paths |

The product sites keep deleting the product location (pack staging stays under
`packs/`, the sandbox temp stays under `<state>/sandbox`); none is redirected to
a generic temp dir. The product-owned delete targets are not optional for the
product, so the fix is a real-path check, not a relocation.

`is_strict_descendant`'s own mutation is not defended by an `assert!`: the
Issue asks for a real-path check on deletion, and the check is a boolean guard,
not the value sent to a syscall. The guard is exercised by its unit tests in
`src/tools/dir_fd.rs` (fake root, sibling canary) and reviewed in the
isolated-container run.

## 3. Detection limits

The inventory test matches **direct API names in `.rs` files**:

- Covered in full: `libc::kill(`, `libc::killpg(`, `nix::sys::signal::kill(`,
  `Command::new("kill"|"killall"|"pkill")`, and `remove_dir_all(`.
- **Missed**: an alias or a re-export of a covered name; a wrapper function that
  hides the token; a signal sent from inside `sh -c` or a Python snippet
  (`python3 -c "import os; os.kill(...)"`); `kill` invoked through a variable or
  `exec`; and any non-Rust execution path (shell scripts, `build.rs` shells out
  but does not signal).
- `grep` counts string literals too (for example
  `tests/gui_read_only_guard.rs`, which asserts the source text). Those are
  declared, so the inventory stays honest about them.

Out of the automated match, with the reason:

- **Single-file delete (`remove_file`)**: many sites (test cleanup and product
  temp files); the dangerous case at Issue #673 scale is the recursive delete,
  and a single `remove_file` cannot remove a tree. Tracked for review, not
  machine-matched.
- **Overwrite / truncate / rename (`File::create`, `O_TRUNC`, `fs::rename`)**:
  high count, mostly correct-by-construction temp staging.
- **Permission changes (`set_permissions`)**: not present in the signal/delete
  threat model of this Issue.
- **Terminal ioctl / termios / shared-fd flags**: measured by Issue #667's own
  probes, not by this inventory.
- **`sh -c` and interpreter bodies**: invisible to a token grep by design; the
  lexical and OS guards cover the write side, the inventory cannot.

The inventory is thus a tripwire for the two categories with the largest blast
radius (signalling every process, and recursive deletion), and an explicit
admission that the remaining categories are not machine-checked here.

## 4. Sites outside this Issue's file scope

`src/planner/runner/tests/support/process.rs`, `tests/headless_summary.rs`, and
`tests/gui_server.rs` contain signal calls but are not in Issue #673's
`## 対象ファイル`, so no `assert!` was added. They are test-only: the value comes
from a child the test itself spawned, and the signal is an existence probe or a
positive-pid `SIGINT`. They are listed so the inventory is complete, and so a
future Issue can decide whether to guard them too.
