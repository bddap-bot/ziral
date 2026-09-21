#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
director_model=$(cat "$here/director-model.txt")
readonly director_model

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
  local events thread text rollout
  events=$(codex exec --skip-git-repo-check --json --output-schema "$schema" --model "$director_model" "$prompt" "${attach[@]}" </dev/null) || return 1
  thread=$(printf '%s\n' "$events" | jq -r 'select(.type == "thread.started") | .thread_id' | head -1)
  [ -n "$thread" ] || { printf '%s\n' "$events" >&2; return 1; }
  rollout=$(find -H "${CODEX_HOME:-$HOME/.codex}/sessions" -name "rollout-*-$thread.jsonl" -print -quit 2>/dev/null)
  [ -s "$rollout" ] || { echo "thread $thread has no rollout" >&2; return 1; }
  jq -se --arg model "$director_model" '
    [.[] | select(.type == "turn_context") | .payload.model]
    | length > 0 and all(. == $model)
  ' "$rollout" >/dev/null || { echo "thread $thread: transcript model must be $director_model" >&2; return 1; }
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
