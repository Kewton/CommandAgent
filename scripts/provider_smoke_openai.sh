#!/usr/bin/env bash
set -euo pipefail

model="${OPENAI_MODEL:-gpt-5.4-mini}"
base_url="${OPENAI_BASE_URL:-https://api.openai.com/v1}"

if [[ -z "${OPENAI_API_KEY:-}" ]]; then
  echo "error: OPENAI_API_KEY is required" >&2
  exit 2
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "error: curl is required for provider smoke" >&2
  exit 2
fi

curl -fsS "$base_url/responses" \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H 'Content-Type: application/json' \
  -d "{\"model\":\"$model\",\"input\":\"Reply with ok.\"}" >/dev/null
echo "openai provider smoke passed: $model"
