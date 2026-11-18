#!/usr/bin/env bash
# Run the Issue #968 groups -- the Recovery obligation (#465) tests and the TUI
# integration tests -- in a local Linux container, the way docs/dev/linux-test-environment.md
# describes.
#
# The test binaries are built on the host with cargo-zigbuild for a Linux target and run
# inside an image that is already present (`--pull=never`); nothing is pulled or built
# into an image. The container runs without network, as a non-root user, and is removed
# when it exits. The record holds only: node presence and version, the build's rustc
# version, LC_ALL/LANG, uid, and each run's test counts. No environment dump, no keys,
# no .env.
#
# Usage: scripts/ci-linux/run-tests.sh <out-dir>
#   CI_LINUX_IMAGE    local image (default node:22)
#   CI_LINUX_TARGET   Linux target for the container's architecture
#                     (default aarch64-unknown-linux-gnu)
#   CI_LINUX_LOCALES  ambient LC_ALL values of the runs (default "C.UTF-8 C")
#   CI_LINUX_REPEAT   runs of each group per locale (default 1)
#   CI_LINUX_SOURCE   source tree to build (default: this repository)
#   CI_LINUX_HIDE_NODE=1  run with /usr/local/bin (the image's node) off PATH, to
#                     observe the missing-node precondition; the groups then fail
#
# Exit 0 only when every run of every group exited 0 with at least one passed test.
set -euo pipefail

out="${1:?usage: scripts/ci-linux/run-tests.sh <out-dir>}"
image="${CI_LINUX_IMAGE:-node:22}"
target="${CI_LINUX_TARGET:-aarch64-unknown-linux-gnu}"
locales="${CI_LINUX_LOCALES:-C.UTF-8 C}"
repeat="${CI_LINUX_REPEAT:-1}"
hide_node="${CI_LINUX_HIDE_NODE:-0}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source_dir="$(cd "${CI_LINUX_SOURCE:-$script_dir/../..}" && pwd)"

for tool in docker cargo-zigbuild python3; do
  command -v "$tool" >/dev/null || {
    echo "missing tool: $tool" >&2
    exit 2
  }
done
docker image inspect "$image" >/dev/null 2>&1 || {
  echo "image $image is not present locally; this script never pulls it" >&2
  exit 2
}

mkdir -p "$out/logs"
out="$(cd "$out" && pwd)"
summary="$out/summary.txt"
: >"$summary"

cd "$source_dir"
target_dir="$(cargo metadata --format-version 1 --no-deps |
  python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')"

{
  echo "source_head=$(git rev-parse HEAD 2>/dev/null || echo unknown)"
  echo "source_changed_files=$(git status --porcelain 2>/dev/null | wc -l | tr -d " ")"
  echo "build_rustc=$(rustc -V)"
  echo "image=$image target=$target locales=$locales repeat=$repeat hide_node=$hide_node"
} >>"$summary"

# Build the lib and tui_integration test binaries for Linux; keep their paths.
cargo-zigbuild test --no-run --target "$target" --lib --test tui_integration \
  --message-format=json >"$out/build.json" 2>"$out/build.err"
python3 - "$out/build.json" >"$out/bins.tsv" <<'PY'
import json, sys
for line in open(sys.argv[1], encoding="utf-8"):
    try:
        item = json.loads(line)
    except ValueError:
        continue
    if item.get("reason") == "compiler-artifact" and item.get("executable") \
            and item.get("profile", {}).get("test"):
        print(item["target"]["name"] + "\t" + item["target"]["kind"][0] + "\t" + item["executable"])
PY
lib_bin="$(awk -F'\t' '$2 == "lib" { print $3 }' "$out/bins.tsv")"
tui_bin="$(awk -F'\t' '$1 == "tui_integration" { print $3 }' "$out/bins.tsv")"
[ -n "$lib_bin" ] && [ -n "$tui_bin" ] || {
  echo "test binaries not found in the build output" >&2
  exit 2
}

name="commandagent-ci-linux-$$"
before="$(docker ps -a -q | wc -l | tr -d ' ')"

# The inner script: one record of the allowed facts, then each group per locale.
# Arguments: lib binary, tui binary, locales, repeat, hide_node. It expands in the container.
# shellcheck disable=SC2016
inner='
set -u
lib=$1; tui=$2; locales=$3; repeat=$4; hide=$5
if [ "$hide" = 1 ]; then PATH=/usr/sbin:/usr/bin:/sbin:/bin; export PATH; fi
if command -v node >/dev/null; then
  echo "node=present version=$(node -v 2>&1)"
else
  echo "node=absent"
fi
echo "uid=$(id -u) ambient_LC_ALL=${LC_ALL-<unset>} ambient_LANG=${LANG-<unset>}"
status=0
for locale in $locales; do
  for i in $(seq 1 "$repeat"); do
    for group in "tui_integration|$tui|" "issue465|$lib|issue465" \
        "recovery_repair_obligation|$lib|recovery_repair_obligation" \
        "tui_terminal|$lib|tui::terminal" "tui_glyphs|$lib|tui::glyphs"; do
      label=${group%%|*}; rest=${group#*|}; bin=${rest%%|*}; filter=${rest#*|}
      log=/out/logs/$label-$locale-$i.log
      if [ -n "$filter" ]; then
        LC_ALL=$locale "$bin" "$filter" >"$log" 2>&1; code=$?
      else
        LC_ALL=$locale "$bin" >"$log" 2>&1; code=$?
      fi
      result=$(grep "^test result" "$log" | tail -n 1)
      passed=$(printf "%s" "$result" | sed -n "s/.* \([0-9]*\) passed.*/\1/p")
      verdict=pass
      if [ "$code" -ne 0 ] || [ -z "$passed" ] || [ "$passed" -eq 0 ]; then
        verdict=fail; status=1
      fi
      echo "run group=$label LC_ALL=$locale i=$i exit=$code verdict=$verdict $result"
    done
  done
done
exit $status
'

set +e
docker run --rm --pull=never --name "$name" --network none --user 1000:1000 \
  -v "$source_dir:$source_dir:ro" -v "$target_dir:$target_dir" -v "$out:/out" \
  -w "$source_dir" "$image" bash -c "$inner" inner \
  "$lib_bin" "$tui_bin" "$locales" "$repeat" "$hide_node" >>"$summary" 2>&1
code=$?
set -e

after="$(docker ps -a -q | wc -l | tr -d ' ')"
left="$(docker ps -a -q --filter "name=$name" | wc -l | tr -d ' ')"
echo "containers before=$before after=$after left_by_this_run=$left" >>"$summary"
echo "docker_exit=$code" >>"$summary"
cat "$summary"
if [ "$left" != 0 ]; then
  echo "a container of this run was left behind: $name" >&2
  exit 1
fi
exit "$code"
