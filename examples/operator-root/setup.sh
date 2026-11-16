#!/bin/bash
# shellcheck shell=bash
#
# Lay out the two-stage example in a directory of its own.
#
#   bash setup.sh <new directory>
#
# Copies the extension, the operator root, the tools, the workspace, and the
# rehearsal files into <new directory>, makes the empty ledgers/, home/, and tmp/
# (the last two are the HOME and TMPDIR the guide runs commandagent with),
# writes the directory's canonical path wherever a descriptor or the run budget
# says @ROOT@, and removes group and other write permission (the host refuses a
# pinned tool directory that others can write). The pins stay as zeros: `commandagent --operator-check --json`
# shows the values to copy in.
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: bash setup.sh <new directory>" >&2
  exit 2
fi

source_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
target=$1

if [ -e "$target" ] && [ -n "$(ls -A "$target")" ]; then
  echo "setup.sh: $target exists and is not empty" >&2
  exit 2
fi
mkdir -p "$target"
root=$(cd "$target" && pwd -P)

# The path is written into JSON and into a sed replacement, so keep it plain.
case "$root" in
  *[!A-Za-z0-9/._-]*)
    echo "setup.sh: use a directory whose path has only letters, digits, '/', '.', '_', and '-'" >&2
    exit 2
    ;;
esac
case "$root/" in
  "$source_dir"/*)
    echo "setup.sh: choose a directory outside the sample itself" >&2
    exit 2
    ;;
esac

for part in extension operator tools workspace rehearse; do
  cp -R "$source_dir/$part" "$root/$part"
done
mkdir -p "$root/ledgers" "$root/home" "$root/tmp"

for file in "$root"/operator/profile-tools/*.json "$root"/operator/run-budgets/*.json; do
  sed "s|@ROOT@|$root|g" "$file" > "$file.new"
  mv "$file.new" "$file"
done

chmod -R go-w "$root"

cat <<EOF
Laid out the example in $root

Next, check the operator root and copy the pins it calculates (step 5 of the
guide; \$ROOT/home and \$ROOT/tmp keep your own settings out of the run):

  env -i PATH="\$PATH" HOME=$root/home TMPDIR=$root/tmp \\
    COMMANDAGENT_ADAPTER_CONFIG_ROOT=$root/operator \\
    COMMANDAGENT_RUN_BUDGET=example-wallet \\
    COMMANDAGENT_COMPATIBLE_REASONING_EFFORT=low \\
    commandagent --operator-check --json --cwd $root/workspace \\
    --extension-root $root/extension --profile two-stage-example \\
    --preset example --base-url http://127.0.0.1:8765/v1 --allow read,write
EOF
