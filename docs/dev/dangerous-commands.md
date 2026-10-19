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
guard is `crate::tools::dir_fd::strict_descendant_real_path(owned_root, target)`:
it `canonicalize`s an `owned_root` that is derived **independently of the target**
and the target, and returns the target's real path only when
`target != root && target.starts_with(root)`. That rejects the root itself, an
ancestor, a sibling, a path outside the root, a `..` that escapes, and a symlink
(even one that points back inside) — none of which `starts_with` alone rejects.
Each caller passes **the returned real path** to `remove_dir_all`, so the checked
value is the value that is removed; a `None` return deletes nothing. Independence
holds on the production `install` path. The test-only `install_for_test` seam is
the one exception: it derives the root from the delete target's parent (see the
note after the table).

| File | Site | Kind | Owned root | Guard |
| --- | --- | --- | --- | --- |
| `src/bin/gui_server/session_paths.rs` | `rollback_unstarted` | product cleanup | the `runs` dir (`SessionPaths.runs_root`) | added |
| `src/model_probe.rs` | `ScratchWorkspace::cleanup` | product cleanup | `std::env::temp_dir()` (`ScratchWorkspace.root`) | added |
| `src/planner/pack/supply.rs` | `install` (failure path) | product staging | `packs/` (`packs_root`) | added |
| `src/planner/pack/supply.rs` | `install` (success path) | product staging | `packs/` | added |
| `src/tools/os_sandbox.rs` | `cleanup` | product temp | `<state>/sandbox` (`Installed.run_tmp_root`) | added — production `install` only; see the note below |
| `src/tools/dir_fd.rs` | test swap seam (×2) | test helper | a synthetic tempdir | **none** — synthetic path |
| `src/tools/edit.rs` | test swap seam | test helper | a synthetic tempdir | **none** — synthetic path |
| `src/tools/write.rs` | test swap seam | test helper | a synthetic tempdir | **none** — synthetic path |
| `tests/gui_server.rs` (×4), `tests/issue505_write_toctou.rs` (×3) | test cleanup | test helper | synthetic tempdirs | **none** — synthetic paths |

Note on `src/tools/os_sandbox.rs`: the production `install` (`install_impl`)
decides `<state>/sandbox` independently of the run temp dir and stores it, so
`cleanup` compares against a reference a mutation of the run temp dir cannot
move. The test-only `install_for_test` seam is not independent — it builds the
root from `policy.run_tmp_dir.parent()`, i.e. from the delete target itself — and
is called only by tests, whose run temp dir is their own tempdir. Taking an
independent root there needs edits to files outside Issue #673's scope
(`src/lib.rs`, `tests/issue502_os_sandbox_network.rs`,
`tests/issue502_os_sandbox_read.rs`), so it is left as a follow-up.

The product sites keep deleting the product location (pack staging stays under
`packs/`, the sandbox temp stays under `<state>/sandbox`); none is redirected to
a generic temp dir. The product-owned delete targets are not optional for the
product, so the fix is a real-path check, not a relocation.

`strict_descendant_real_path`'s own mutation is not defended by an `assert!`:
the Issue asks for a real-path check on deletion, and the check guards a delete,
not a syscall target. Returning `Option<PathBuf>` keeps the mutation direction
safe — a body-replacement mutant yields `None` (the `Option` default), which
deletes nothing — and the guard is exercised by its unit tests in
`src/tools/dir_fd.rs` (fake root, sibling canary) and reviewed in the
isolated-container run.

## 2a. Shell and Python sites

The inventory also reads tracked `scripts/**/*.sh`, `scripts/**/*.py`, and
`tests/**/*.py`. The declared sites it found:

| File | Token | Count | Notes |
| --- | --- | --- | --- |
| `scripts/build-release.sh` | `rm -rf` | 3 | build/publish dir cleanup |
| `scripts/install.sh` | `rm -rf` | 1 | temp dir cleanup |
| `scripts/cmate-pm-watch.sh` | `rm -rf` | 3 | stop-pattern string and its own grep |
| `scripts/demo/record_cli_demo.py` | `os.kill(` | 1 | terminate a recorded child |
| `scripts/eval_lib/heavy_lock.py` | `os.kill(` | 2 | liveness probe (`signal 0`) |
| `scripts/eval_lib/heavy_lock.py` | `shutil.rmtree(` | 2 | lock dir cleanup |
| `scripts/eval_lib/goal_verify_recovery_deterministic_pair.py` | `os.killpg(` | 2 | terminate a spawned process group |
| `scripts/eval_lib/goal_verify_recovery_exposure_corpus_pilot.py` | `os.killpg(` | 2 | terminate a spawned process group |
| `scripts/eval_lib/goal_verify_workspaces_v3.py` | `shutil.rmtree(` | 1 | destination cleanup |
| `scripts/eval_lib/goal_verify_live_v4.py` | `shutil.rmtree(` | 2 | destination cleanup |
| `scripts/nextjs_domain_oracle.py` | `shutil.rmtree(` | 1 | scratch cleanup |
| `tests/cli_cases/harness.py` | `os.killpg(` | 1 | reap the process group a providerless CLI case started (`start_new_session`, so the group is the case's own); the helper refuses pgid <= 1 and the harness's own group before the call, and treats an already-gone group as done |
| `tests/eval/support/next_ready.py` | `os.killpg(` | 1 | stop the opt-in Next.js `npm start` group (SIGTERM, then SIGKILL) only while its leader is unreaped; `assert pgid > 1` directly before the call; a reaped leader gets no signal |
| `tests/eval/test_heavy_lock.py` | `os.kill(` | 1 | liveness probe (`signal 0`) |

No external `kill`/`pkill`/`killall` or `rm -r ` occurs anywhere in the scanned
set; every `rm -rf`, `os.kill(`, `os.killpg(`, and `shutil.rmtree(` site is
listed above.

## 3. Detection limits

The inventory test matches **direct API names and shell commands in tracked `.rs`,
`.sh`, and `.py` files**:

- Rust (`src/**`, `tests/**`, `build.rs`): `libc::kill(`, `libc::killpg(`,
  `nix::sys::signal::kill(`, `Command::new("kill"|"killall"|"pkill")`, and
  `remove_dir_all(`.
- Shell (`scripts/**/*.sh`): external `kill `, `pkill`, `killall`, `rm -rf`,
  `rm -r `.
- Python (`scripts/**/*.py`, `tests/**/*.py`): `os.kill(`, `os.killpg(`,
  `shutil.rmtree(`.
- **Missed**: an alias or a re-export of a covered name; a wrapper function that
  hides the token; an external `kill` not followed by a space (for example
  `kill$pid` or a bare `kill` on its own line); a signal sent from inside `sh -c`
  or a Python string (`python3 -c "import os; os.kill(...)"`); `kill` invoked
  through a variable or `exec`; Node's `child.kill()` in `scripts/*.mjs`; and any
  other language not scanned.
- `grep` counts string literals too (for example
  `tests/gui_read_only_guard.rs`, which asserts the source text, and
  `scripts/cmate-pm-watch.sh`, whose stop-pattern string contains `rm -rf`).
  Those are declared, so the inventory stays honest about them.

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
- **`sh -c` and interpreter string bodies**: invisible to a token grep by design;
  the lexical and OS guards cover the write side, the inventory cannot.
- **Node/`.mjs` scripts**: `scripts/issue45*_*.mjs` call `server.kill('SIGTERM')`
  on a child the script itself spawned. The token set the Issue names does not
  include Node's `ChildProcess.kill()`, so these are outside the match.

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

## 5. Dangerous-value tests (mutated runs only in the isolated container)

These pass dangerous values (0, 1, negative) to a function that calls `kill`.
They must not run on the host **under a mutation** — neither inside a `cargo
mutants` run nor on a tree where the defense was removed by hand; that is the
Mutation-testing rule in `parallel-dev-harness.md` §3a. On an unmutated tree they
do run on the host (CI and the verify gates do not skip them): the
`Err`/`bail!`/`return false` path returns before `libc::kill`, so no syscall
happens. A newly added `dangerous_value_` test is first confirmed once in the
isolated container on an unmutated tree, before it may run on the host. The
`dangerous_value_` tests deliberately do not check the selector, so under a
mutated selector the value reaches the non-mutatable `assert!` in front of `kill`
and panics there (the Issue #673 対処の方向 4 requirement), and no syscall is
made.

| Test | Target | Notes |
| --- | --- | --- |
| `bounded_process::tests::process_group_guard_refuses_non_child_group_ids` | `terminate_process_group_by_pid` | existing; also checks the selector |
| `bounded_process::tests::dangerous_value_terminate_process_group_by_pid_refuses_init_and_caller_group` | `terminate_process_group_by_pid(0/1)` | new; skips the selector, so a mutated selector reaches the `assert!` |
| `preflight::tests::dangerous_value_system_port_control_kill_refuses_init_zero_and_own_pid` | `SystemPortControl::kill(0/1/own pid)` | new; on an unmutated tree the `bail!` paths return `Err` before `libc::kill` |
| `bounded_process::tests::dangerous_sentinel_same_uid_other_group_survives` | real `kill` scope | `#[ignore]`, needs `COMMANDAGENT_ISSUE673_CONTAINER=1` |
| `trial_process::tests::process_group_guard_rejects_init_and_caller_group` | `send_group_signal` / `process_group_exists` | existing; also checks the selector; `--features gui` |
| `trial_process::tests::dangerous_value_send_group_signal_refuses_init_caller_and_negative_groups` | `send_group_signal(0/1/-1)`, `process_group_exists(0/1/-1)` | new; skips the selector; `--features gui` |

Docker commands (from a container that shares no host PID namespace, is not
`--privileged`, and shares no host HOME, terminal, or Docker socket):

```sh
cargo test --lib -- --exact bounded_process::tests::process_group_guard_refuses_non_child_group_ids
cargo test --lib -- --exact bounded_process::tests::dangerous_value_terminate_process_group_by_pid_refuses_init_and_caller_group
cargo test --lib -- --exact preflight::tests::dangerous_value_system_port_control_kill_refuses_init_zero_and_own_pid
COMMANDAGENT_ISSUE673_CONTAINER=1 cargo test --lib -- --ignored --exact \
  bounded_process::tests::dangerous_sentinel_same_uid_other_group_survives
cargo test --features gui --bin gui_server -- --exact \
  trial_process::tests::process_group_guard_rejects_init_and_caller_group
cargo test --features gui --bin gui_server -- --exact \
  trial_process::tests::dangerous_value_send_group_signal_refuses_init_caller_and_negative_groups
```
