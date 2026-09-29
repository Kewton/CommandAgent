#!/usr/bin/env bash
# PM monitor for one CommandMate session in the parallel-dev harness
# (docs/dev/parallel-dev-harness.md, sections 4 and 6). Run it in the
# background after each brief; it exits with one status line:
#
#   READY            the report file exists and the DONE line is on screen
#   ESCALATE         a prompt is open that this monitor may not answer
#   ESCALATE_UNSEEN  auto-yes is off and a stop-pattern command is on screen
#   STALLED          the session hit its context limit
#   NO_PROGRESS      the pane has not changed for --stall-minutes
#   TIMEOUT          --timeout-minutes elapsed
#
# While running, it answers a prompt only when the command matches --allow
# (and, for recursive deletes, only under --rm-root), then re-enables auto-yes
# with the harness stop pattern. Every automatic action is logged to --ledger.
set -uo pipefail

usage() {
  cat <<'EOF'
usage: cmate-pm-watch.sh --instance ID --report PATH --done REGEX --ledger PATH
                         [--worktree ID] [--allow REGEX] [--rm-root DIR]
                         [--stall-minutes N] [--timeout-minutes N] [--interval S]
EOF
}

WT=commandagent-develop
INST=""
REPORT=""
DONE_RE=""
LEDGER=""
ALLOW_RE=""
RM_ROOT=""
STALL_MIN=20
TIMEOUT_MIN=360
INTERVAL=20
CM=${CM:-commandmate}

while [ $# -gt 0 ]; do
  case "$1" in
    --worktree) WT=$2; shift 2 ;;
    --instance) INST=$2; shift 2 ;;
    --report) REPORT=$2; shift 2 ;;
    --done) DONE_RE=$2; shift 2 ;;
    --ledger) LEDGER=$2; shift 2 ;;
    --allow) ALLOW_RE=$2; shift 2 ;;
    --rm-root) RM_ROOT=$2; shift 2 ;;
    --stall-minutes) STALL_MIN=$2; shift 2 ;;
    --timeout-minutes) TIMEOUT_MIN=$2; shift 2 ;;
    --interval) INTERVAL=$2; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
  esac
done
if [ -z "$INST" ] || [ -z "$REPORT" ] || [ -z "$DONE_RE" ] || [ -z "$LEDGER" ]; then
  usage >&2
  exit 2
fi

STOP='git push|gh pr (create|merge)|cargo publish|git reset --hard|git clean -[a-z]*f|rm -rf|commandmate (start|stop|init|auto-yes)|source \./\.env'
# Never answered automatically, even when --allow matches.
FORBID='--force|push -f|--admin|git reset --hard|git clean|commandmate (start|stop|init)|\.env|cargo publish'
BUSY='Working \(|esc to interrupt'
PROMPT='1\. Yes|Do you want|Allow |Would you like'
LIMIT='maximum context length|Type "continue" to try again|prompt is too long'

log() {
  printf '| %s | auto | %s | %s | — | %s |\n' "$(date '+%F %T')" "$INST" "$1" "${2:-}" >> "$LEDGER"
}

capture() {
  $CM capture "$WT" --instance "$INST" --pane --tail 40 2>/dev/null \
    | sed $'s/\x1b\\[[0-9;?]*[A-Za-z]//g; s/\x1b\\]8;;[^\x1b]*\x1b\\\\//g'
}

auto_yes_state() {
  $CM instances "$WT" --json 2>/dev/null \
    | jq -r --arg i "$INST" '.[] | select(.instanceId == $i) | .autoYes'
}

enable_auto_yes() {
  $CM auto-yes "$WT" --instance "$INST" --enable --duration 8h --stop-pattern "$STOP" >/dev/null 2>&1
}

pane_cpu() {
  local session pid
  session=$($CM instances "$WT" --json 2>/dev/null \
    | jq -r --arg i "$INST" '.[] | select(.instanceId == $i) | .tmuxSession')
  pid=$(tmux list-panes -t "$session" -F '#{pane_pid}' 2>/dev/null | head -1)
  [ -n "$pid" ] || return 0
  # The pane shell, its children, and their children (the agent binary).
  ps -A -o pid=,ppid=,pcpu=,comm= | awk -v root="$pid" '
    { ppid[$1] = $2; line[$1] = $0 }
    END { for (p in line) if (p == root || ppid[p] == root || ppid[ppid[p]] == root) print line[p] }'
}

# Every recursive delete in the command must target a path under --rm-root.
rm_targets_ok() {
  local cmd=$1 target
  printf '%s' "$cmd" | grep -q 'rm -rf' || return 0
  [ -n "$RM_ROOT" ] || return 1
  while read -r target; do
    case "$target" in
      "$RM_ROOT"/*) ;;
      *) return 1 ;;
    esac
  done < <(printf '%s' "$cmd" | grep -oE 'rm -rf +[^ ;&|]+' | awk '{print $3}' | tr -d "\"'")
  return 0
}

end=$((SECONDS + TIMEOUT_MIN * 60))
last_hash=""
last_change=$SECONDS
while [ "$SECONDS" -lt "$end" ]; do
  out=$(capture)
  tail25=$(printf '%s\n' "$out" | tail -25)
  # Long commands wrap in the pane; match on whitespace-normalized text.
  flat=$(printf '%s' "$tail25" | tr -s ' \n' ' ')

  if [ -f "$REPORT" ] && printf '%s' "$out" | grep -Eq "$DONE_RE" \
    && ! printf '%s' "$tail25" | grep -Eq "$BUSY"; then
    echo READY
    exit 0
  fi
  if printf '%s' "$tail25" | grep -Eq "$LIMIT"; then
    echo STALLED
    printf '%s\n' "$tail25" | tail -12
    exit 0
  fi

  if [ "$(auto_yes_state)" = "false" ]; then
    if printf '%s' "$tail25" | tail -15 | grep -Eq "$PROMPT"; then
      if [ -n "$ALLOW_RE" ] && printf '%s' "$flat" | grep -Eq "$ALLOW_RE" \
        && ! printf '%s' "$flat" | grep -Eq -- "$FORBID" && rm_targets_ok "$flat"; then
        $CM respond "$WT" "1" --instance "$INST" >/dev/null 2>&1
        enable_auto_yes
        log "停止パターン（範囲内）→ respond 1・auto-yes 再有効化" \
          "\`$(printf '%s' "$flat" | tail -c 160 | tr '|' '/')\`"
        sleep 8
        continue
      fi
      echo ESCALATE
      printf '%s\n' "$tail25" | tail -16
      exit 0
    fi
    # No prompt visible: re-enabling now would let auto-yes answer a
    # stop-pattern command the PM never saw.
    if printf '%s' "$flat" | grep -Eq "$STOP"; then
      echo ESCALATE_UNSEEN
      printf '%s\n' "$tail25" | tail -16
      exit 0
    fi
    enable_auto_yes
    log "auto-yes が off（確認なし）→ 再有効化"
  fi

  hash=$(printf '%s' "$out" | shasum | cut -c1-16)
  if [ "$hash" != "$last_hash" ]; then
    last_hash=$hash
    last_change=$SECONDS
  elif [ $((SECONDS - last_change)) -ge $((STALL_MIN * 60)) ]; then
    echo NO_PROGRESS
    printf '%s\n' "$tail25" | tail -10
    pane_cpu
    exit 0
  fi
  sleep "$INTERVAL"
done
echo TIMEOUT
