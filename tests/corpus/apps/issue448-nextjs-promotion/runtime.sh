#!/bin/sh
# Test-only npm transport: original commands and source inputs remain unchanged.
set -eu
runtime=$(CDPATH= cd -- "$(dirname -- "$0")/../.issue448" && pwd)
[ "$#" -eq 2 ] && [ "$1" = run ] || exit 90
mkdir -p .commandagent/evidence
case "$2" in
  build)
    while IFS= read -r relative; do
      cmp "$runtime/expected/$relative" "$relative" || exit 91
    done < "$runtime/inputs.txt"
    printf '%s\n' 'npm run build' >> .commandagent/evidence/issue448-commands.txt
    # Existing cfg(test) interaction seams are read only after readiness/build.
    # These are scripted input responses, not final browser acceptance records.
    mkdir -p .anvil/evidence
    cp "$runtime/availability.json" .anvil/evidence/interaction-probe-availability.json
    cp "$runtime/interaction.json" .anvil/evidence/interaction-probe-result.json
    if [ -f "$runtime/real-npm.txt" ]; then
      real_npm=$(cat "$runtime/real-npm.txt")
      mode=real
      if NEXT_TELEMETRY_DISABLED=1 "$real_npm" run build > .commandagent/evidence/issue448-build-output.txt 2>&1; then
        code=0
      else
        code=$?
      fi
      cat .commandagent/evidence/issue448-build-output.txt
    else
      cat "$runtime/build-diagnostics.txt"
      mode=measured-replay
      code=$(cat "$runtime/build-exit.txt")
    fi
    printf 'npm run build [%s] exit=%s\n' "$mode" "$code" >> .commandagent/evidence/issue448-commands.txt
    exit "$code"
    ;;
  start)
    [ "$PORT" = 60302 ] || exit 92
    printf '%s\n' 'npm run start' >> .commandagent/evidence/issue448-commands.txt
    test_exe=$(cat "$runtime/test-exe.txt")
    status=$(cat "$runtime/http-status.txt")
    exec env COMMANDAGENT_BROWSER_PROBE_MOCK_CHILD=1 \
      COMMANDAGENT_BROWSER_PROBE_MOCK_PORT="$PORT" \
      COMMANDAGENT_BROWSER_PROBE_MOCK_STATUS="$status" \
      "$test_exe" --ignored --exact \
      minimal_loop::browser_probe::tests::browser_probe_mock_server_child --nocapture
    ;;
  *) exit 93 ;;
esac
