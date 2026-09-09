#!/usr/bin/env bash
set -euo pipefail

attach=()
while getopts 'i:' opt; do
  case $opt in
  i) attach+=(--image "$(realpath "$OPTARG")") ;;
  *) echo "usage: critic.sh [-i IMAGE]... PROMPT" >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
[ $# -eq 1 ] || { echo "usage: critic.sh [-i IMAGE]... PROMPT" >&2; exit 2; }
prompt="$1 Answer with exactly one JSON object and nothing else, {\"score\": an integer from 0 to 10, \"issues\": a list of at most five strings, most important first, each one sentence naming what is wrong and where}; do not run commands, edit anything or write files."
schema=$(mktemp)
trap 'rm -f "$schema"' EXIT
cat > "$schema" <<'JSON'
{"type":"object","properties":{"score":{"type":"integer","minimum":0,"maximum":10},"issues":{"type":"array","maxItems":5,"items":{"type":"string"}}},"required":["score","issues"],"additionalProperties":false}
JSON

one() {
  local events thread text
  events=$(codex exec --skip-git-repo-check --ephemeral --json --output-schema "$schema" "$prompt" "${attach[@]}" </dev/null)
  thread=$(printf '%s\n' "$events" | jq -r 'select(.type == "thread.started") | .thread_id' | head -1)
  [ -n "$thread" ] || { printf '%s\n' "$events" >&2; return 1; }
  text=$(printf '%s\n' "$events" | jq -r 'select(.type == "item.completed" and .item.type == "agent_message") | .item.text' | tail -1)
  [ -n "$text" ] || { echo "thread $thread returned no message" >&2; return 1; }
  printf '%s\n' "$text"
}

attempts=4
for attempt in $(seq 1 "$attempts"); do
  one && exit 0
  if [ "$attempt" -eq "$attempts" ]; then
    echo "critic: gave up after $attempts attempts" >&2
    exit 1
  fi
  wait=$((1 << (attempt - 1)))
  echo "critic: attempt $attempt of $attempts failed, retrying in ${wait}s" >&2
  sleep "$wait"
done
