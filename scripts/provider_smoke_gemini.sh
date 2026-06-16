#!/usr/bin/env bash
set -euo pipefail

model="${GEMINI_MODEL:-gemini-3.5-flash}"
base_url="${GEMINI_BASE_URL:-https://generativelanguage.googleapis.com/v1beta}"

if [[ -z "${GEMINI_API_KEY:-}" ]]; then
  echo "error: GEMINI_API_KEY is required" >&2
  exit 2
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "error: curl is required for provider smoke" >&2
  exit 2
fi

curl -fsS "$base_url/models/$model:generateContent?key=$GEMINI_API_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"contents":[{"parts":[{"text":"Reply with ok."}]}]}' >/dev/null
echo "gemini provider smoke passed: $model"
