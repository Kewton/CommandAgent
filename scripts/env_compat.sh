#!/usr/bin/env bash

_commandagent_warned_legacy_names="|"

commandagent_env_get() {
  local output_name="$1"
  local current_name="$2"
  local default_value="${3-}"
  if declare -p "$current_name" >/dev/null 2>&1; then
    printf -v "$output_name" '%s' "${!current_name}"
    return
  fi

  local suffix="${current_name#COMMANDAGENT_}"
  local legacy_name="ANVIL_${suffix}"
  if ! declare -p "$legacy_name" >/dev/null 2>&1; then
    printf -v "$output_name" '%s' "$default_value"
    return
  fi

  case "$_commandagent_warned_legacy_names" in
    *"|${legacy_name}|"*) ;;
    *)
      printf 'warning: %s is deprecated; use %s instead\n' "$legacy_name" "$current_name" >&2
      _commandagent_warned_legacy_names="${_commandagent_warned_legacy_names}${legacy_name}|"
      ;;
  esac
  printf -v "$output_name" '%s' "${!legacy_name}"
}
