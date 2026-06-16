#!/usr/bin/env bash
set -euo pipefail

runs=1
dry_run=0
extra_args=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release-quality)
      runs=3
      shift
      ;;
    --runs)
      runs="$2"
      shift 2
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    *)
      extra_args+=("$1")
      shift
      ;;
  esac
done

args=(
  --cases-dir eval/cases/large
  --runs "$runs"
)

if [[ "$dry_run" == "1" ]]; then
  args+=(--dry-run)
fi

scripts/eval_agent_slice.sh "${args[@]}" "${extra_args[@]}"
