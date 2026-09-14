#!/bin/sh
# Test transport only. Build/interaction inputs are scripted, not live evidence.
set -eu
runtime=$(CDPATH= cd -- "$(dirname -- "$0")/../.issue485" && pwd)
[ "$#" -eq 2 ] && [ "$1" = run ] || exit 90
mkdir -p .commandagent/evidence
case "$2" in
  build)
    while IFS= read -r relative; do
      cmp "$runtime/expected/$relative" "$relative" || exit 91
    done < "$runtime/inputs.txt"
    printf '%s\n' 'npm run build: scripted input' >> .commandagent/evidence/issue485-commands.txt
    mkdir -p .anvil/evidence
    cp "$runtime/availability.json" .anvil/evidence/interaction-probe-availability.json
    cp "$runtime/interaction.json" .anvil/evidence/interaction-probe-result.json
    code=$(cat "$runtime/build-exit.txt")
    if [ "$code" != 0 ]; then printf '%s\n' 'Failed to compile: scripted build failure' >&2; fi
    exit "$code"
    ;;
  start)
    [ "$PORT" = "$(cat "$runtime/transport-port.txt")" ] || exit 92
    printf '%s\n' 'npm run start: scripted HTTP input' >> .commandagent/evidence/issue485-commands.txt
    test_exe=$(cat "$runtime/test-exe.txt")
    exec env COMMANDAGENT_BROWSER_PROBE_MOCK_CHILD=1 \
      COMMANDAGENT_BROWSER_PROBE_MOCK_PORT="$PORT" \
      COMMANDAGENT_BROWSER_PROBE_MOCK_STATUS=200 \
      "$test_exe" --ignored --exact \
      minimal_loop::browser_probe::tests::browser_probe_mock_server_child --nocapture
    ;;
  *) exit 93 ;;
esac
