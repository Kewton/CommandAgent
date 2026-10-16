#!/usr/bin/env bash
set -euo pipefail

report_tiocsti_guidance() {
  local val=""
  local tiocsti_str=""
  if command -v sysctl >/dev/null 2>&1; then
    val="$(sysctl -n dev.tty.legacy_tiocsti 2>/dev/null || true)"
  fi
  if [ -n "$val" ]; then
    tiocsti_str="$val"
  else
    tiocsti_str="could not be read (sysctl unavailable or dev.tty.legacy_tiocsti not present)"
  fi
  echo "dev.tty.legacy_tiocsti might not have been set. Check the logs of step 'Enable legacy TIOCSTI for the isolation probes'."
  echo "Current dev.tty.legacy_tiocsti: $tiocsti_str"
}

verdicts="${1:-${COMMANDAGENT_PROBE_VERDICT_FILE:-}}"

if [ -z "$verdicts" ]; then
  echo "::error::isolation probe verdict file path was not provided (pass as first argument or set COMMANDAGENT_PROBE_VERDICT_FILE)"
  report_tiocsti_guidance
  exit 1
fi

if [ ! -e "$verdicts" ]; then
  echo "::error::isolation probe verdict file does not exist: $verdicts"
  report_tiocsti_guidance
  exit 1
fi

if [ ! -s "$verdicts" ]; then
  echo "::error::no isolation probe verdict was recorded in $verdicts: the probes did not run, or the file is empty"
  report_tiocsti_guidance
  exit 1
fi

echo "isolation probe verdicts ($verdicts):"
cat "$verdicts"

has_unverified=0
while IFS= read -r line || [ -n "$line" ]; do
  case "$line" in
    *[Uu][Nn][Vv][Ee][Rr][Ii][Ff][Ii][Ee][Dd]*)
      echo "::error::$line"
      has_unverified=1
      ;;
  esac
done < "$verdicts"

if [ "$has_unverified" -ne 0 ]; then
  report_tiocsti_guidance
  exit 1
fi
