#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v rg >/dev/null 2>&1; then
  echo "error: ripgrep is required for branding checks" >&2
  exit 2
fi

pattern='anvil|Anvil|ANVIL|\.anvil|anvil>'

matches="$(
  rg -n --hidden --glob '!target/**' --glob '!.git/**' "$pattern" . \
    | grep -v '^./scripts/check_branding.sh:' || true
)"

if [[ -n "$matches" ]]; then
  echo "error: old brand references found" >&2
  echo "$matches" >&2
  exit 1
fi

echo "branding check passed"
