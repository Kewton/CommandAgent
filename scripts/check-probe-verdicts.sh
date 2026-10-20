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

# Check one required probe id line by line: it must appear exactly once and its
# verdict (the first word after "<id>:") must be one of the accepted words. A
# missing id, a duplicate, an empty verdict and an unknown verdict all fail. An
# UNVERIFIED verdict is already reported above and is never accepted here.
# MPTCP ALLOWED stays an accepted, recorded kernel limit; it is not read as a
# refusal.
has_required_error=0
check_required() {
  local id="$1"
  local accepted=" $2 "
  local count=0
  local verdict=""
  local line rest
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      "$id:"*)
        count=$((count + 1))
        rest="${line#"$id:"}"
        rest="${rest#"${rest%%[![:space:]]*}"}"
        verdict="${rest%%[[:space:];(]*}"
        ;;
    esac
  done < "$verdicts"
  if [ "$count" -eq 0 ]; then
    echo "::error::required isolation probe verdict is missing: $id"
    has_required_error=1
  elif [ "$count" -gt 1 ]; then
    echo "::error::required isolation probe verdict is recorded $count times: $id"
    has_required_error=1
  elif [ -z "$verdict" ]; then
    echo "::error::required isolation probe verdict is empty: $id"
    has_required_error=1
  else
    case "$accepted" in
      *" $verdict "*) ;;
      *)
        echo "::error::required isolation probe verdict is not accepted: $id: $verdict"
        has_required_error=1
        ;;
    esac
  fi
}

check_required "issue667/tiocsti" "blocked"
check_required "issue672/mptcp" "REFUSED ALLOWED"

if [ "$has_unverified" -ne 0 ] || [ "$has_required_error" -ne 0 ]; then
  report_tiocsti_guidance
  exit 1
fi
