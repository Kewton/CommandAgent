#!/usr/bin/env bash
# Run the ten pseudo-terminal cases of tests/tui_pty.rs by name and check the result.
#
# The cases are #[ignore]d and return early (counting as a pass) unless
# COMMANDAGENT_PTY_TESTS=1, so this script sets it, selects the cases with --exact, and
# refuses to report success unless all of these hold:
#   - a pseudo-terminal can be opened through script(1), the helper the cases use;
#   - the ignored cases listed by tests/tui_pty.rs are exactly the ten expected names;
#   - cargo exits 0 and libtest reports 10 passed, 0 failed, 0 ignored;
#   - every expected name printed "ok".
# No TTY (exit 3), a wrong count or name set (exit 4), a timeout (exit 5) and a failed case
# (exit 1) are all non-zero; none is counted as a pass.
set -euo pipefail

expected=(
  tui_pty_smoke
  tui_pty_labels_gate_one_classifier_without_planning_wording
  tui_pty_warns_when_ollama_is_stopped_and_keeps_prompt_usable
  tui_pty_warns_when_ollama_model_is_missing_and_keeps_prompt_usable
  tui_pty_queues_input_during_command_and_replays_fifo
  tui_pty_scopes_history_and_supports_session_controls
  tui_pty_suppresses_planner_stream_with_spinner_and_footer_cleanup
  tui_pty_planner_stream_interrupt_cleans_spinner_footer_and_status
  tui_pty_planner_interrupt_closes_http_and_reaches_gate_four_within_one_second
  tui_pty_screen_state_preserves_long_accepted_goal_across_footer_modes
)
expected_count=10
suite_timeout_secs="${PTY_SUITE_TIMEOUT_SECS:-1500}"

export COMMANDAGENT_PTY_TESTS=1
export RUSTFLAGS="${RUSTFLAGS:--D warnings}"

log="$(mktemp)"
trap 'rm -f "$log"' EXIT

unverified() {
  echo "pty: UNVERIFIED: $1" >&2
  exit 3
}

# The cases open their pseudo-terminal through script(1) with the same arguments as
# tests/support/pty_wait.rs::script_command, so probe that exact form.
echo "pty: platform: $(uname -s)"
command -v script >/dev/null 2>&1 || unverified "script(1) is not installed"
if [ "$(uname -s)" = "Darwin" ]; then
  probe="$(script -q /dev/null /bin/sh -c 'if [ -t 0 ] && [ -t 1 ]; then echo PTY_PROBE_TTY; else echo PTY_PROBE_NOTTY; fi' </dev/null 2>&1 || true)"
else
  probe="$(script -q -c 'if [ -t 0 ] && [ -t 1 ]; then echo PTY_PROBE_TTY; else echo PTY_PROBE_NOTTY; fi' /dev/null </dev/null 2>&1 || true)"
fi
case "$probe" in
  *PTY_PROBE_TTY*) echo "pty: tty: available (script(1) gave the child a terminal on stdin and stdout)" ;;
  *) unverified "script(1) did not give the child a terminal (probe output: $probe)" ;;
esac

listed="$(cargo test --test tui_pty -- --list --ignored | sed -n 's/^\(tui_pty_[A-Za-z0-9_]*\): test$/\1/p' | sort)"
wanted="$(printf '%s\n' "${expected[@]}" | sort)"
listed_count="$(printf '%s' "$listed" | grep -c . || true)"
echo "pty: expected cases: $expected_count; ignored cases listed by tests/tui_pty.rs: $listed_count"
if [ "$listed" != "$wanted" ]; then
  echo "pty: the listed ignored cases differ from the expected names (< expected, > listed):" >&2
  diff <(printf '%s\n' "$wanted") <(printf '%s\n' "$listed") >&2 || true
  exit 4
fi

timeout_cmd=()
if command -v timeout >/dev/null 2>&1; then
  timeout_cmd=(timeout "$suite_timeout_secs")
elif command -v gtimeout >/dev/null 2>&1; then
  timeout_cmd=(gtimeout "$suite_timeout_secs")
else
  echo "pty: warning: no timeout command; the suite timeout is not enforced by this script"
fi
echo "pty: running $expected_count cases by exact name (suite timeout ${suite_timeout_secs}s)"
printf 'pty: case: %s\n' "${expected[@]}"

set +e
${timeout_cmd[@]+"${timeout_cmd[@]}"} cargo test --test tui_pty -- --ignored --exact "${expected[@]}" 2>&1 | tee "$log"
status="${PIPESTATUS[0]}"
set -e

if [ "$status" -eq 124 ] || [ "$status" -eq 137 ]; then
  echo "pty: TIMEOUT after ${suite_timeout_secs}s; not counted as a pass" >&2
  exit 5
fi
if [ "$status" -ne 0 ]; then
  echo "pty: FAILED: cargo test exited $status" >&2
  exit 1
fi

for name in "${expected[@]}"; do
  if ! grep -qx "test $name ... ok" "$log"; then
    echo "pty: case did not report ok: $name" >&2
    exit 4
  fi
done
if ! grep -q "^test result: ok\. $expected_count passed; 0 failed; 0 ignored; 0 measured; " "$log"; then
  echo "pty: libtest did not report exactly $expected_count passed, 0 failed, 0 ignored" >&2
  exit 4
fi
echo "pty: result: $expected_count of $expected_count cases passed with a working TTY"
