#!/usr/bin/env bash
set -euo pipefail

attach=()
while getopts 'i:' opt; do
  case $opt in
  i) attach+=(--image "$(realpath "$OPTARG")") ;;
  *) echo "usage: ask.sh [-i IMAGE]... SCHEMA PROMPT" >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
[ $# -eq 2 ] || { echo "usage: ask.sh [-i IMAGE]... SCHEMA PROMPT" >&2; exit 2; }
schema=$(mktemp)
trap 'rm -f "$schema"' EXIT
printf '%s\n' "$1" > "$schema"
prompt=$2

one() {
  local events thread text
  events=$(codex exec --skip-git-repo-check --ephemeral --json --output-schema "$schema" "$prompt" "${attach[@]}" </dev/null)
  thread=$(printf '%s\n' "$events" | jq -r 'select(.type == "thread.started") | .thread_id' | head -1)
  [ -n "$thread" ] || { printf '%s\n' "$events" >&2; return 1; }
  text=$(printf '%s\n' "$events" | jq -rRs '[splits("\n") | select(length > 0) | fromjson? | select(.type == "item.completed" and .item.type == "agent_message")] | last | .item.text // empty')
  [ -n "$text" ] || { echo "thread $thread returned no message" >&2; return 1; }
  printf '%s\n' "$text"
}

attempts=4
for attempt in $(seq 1 "$attempts"); do
  one && exit 0
  if [ "$attempt" -eq "$attempts" ]; then
    echo "ask: gave up after $attempts attempts" >&2
    exit 1
  fi
  wait=$((1 << (attempt - 1)))
  echo "ask: attempt $attempt of $attempts failed, retrying in ${wait}s" >&2
  sleep "$wait"
done
