#!/usr/bin/env bash
set -euo pipefail

host="${OLLAMA_HOST:-http://127.0.0.1:11434}"

if ! command -v curl >/dev/null 2>&1; then
  echo "error: curl is required for provider smoke" >&2
  exit 2
fi

curl -fsS "$host/api/tags" >/dev/null
echo "ollama provider smoke passed: $host/api/tags"
