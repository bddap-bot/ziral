#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

size=1024
attach=()
while getopts 's:i:' opt; do
  case $opt in
  s) size=$OPTARG ;;
  i) attach+=(--image "$(realpath "$OPTARG")") ;;
  *) echo "usage: paint.sh [-s SIZE] [-i IMAGE]... OUT.png PROMPT" >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
[ $# -eq 2 ] || { echo "usage: paint.sh [-s SIZE] [-i IMAGE]... OUT.png PROMPT" >&2; exit 2; }
target=$1
subject=$2
prompt="Generate exactly one square image with the image generation tool, every attached image as its reference, and the text between the markers below as the tool's prompt, passed unchanged, character for character, with nothing added before or after it, then stop: do not judge, retry, edit, or describe the image, and write no files.
<<<PROMPT
$subject
PROMPT>>>"

received() {
  local thread=$1 rollout calls
  rollout=$(find "$HOME/.codex/sessions" -name "rollout-*-$thread.jsonl" -print -quit)
  [ -s "$rollout" ] || { echo "thread $thread has no rollout" >&2; return 1; }
  calls=$(jq -r 'select(.type == "response_item" and .payload.type == "custom_tool_call" and (.payload.input | type) == "string") | .payload.input | select(test("image_gen__imagegen")) | capture("\"?prompt\"?\\s*:\\s*(?<p>\"(?:[^\"\\\\]|\\\\.)*\")") | .p | fromjson | tojson' "$rollout")
  [ -n "$calls" ] || { echo "thread $thread made no image tool call" >&2; return 1; }
  printf '%s\n' "$calls"
}

one() {
  local events thread srcs w h got
  events=$(codex exec --skip-git-repo-check --json "$prompt" "${attach[@]}" </dev/null)
  thread=$(printf '%s\n' "$events" | jq -r 'select(.type == "thread.started") | .thread_id' | head -1)
  [ -n "$thread" ] || { printf '%s\n' "$events" >&2; return 1; }
  srcs=("$HOME/.codex/generated_images/$thread"/*.png)
  [ "${#srcs[@]}" -eq 1 ] || { echo "thread $thread holds ${#srcs[@]} images, not one" >&2; return 1; }
  got=$(received "$thread") || return 1
  printf '%s\n' "$got" | grep -qxF -- "$(jq -Rsr 'rtrimstr("\n") | tojson' <<<"$subject")" || { printf 'thread %s: the image tool received another prompt:\n%s\n' "$thread" "$(printf '%s\n' "$got" | jq -r .)" >&2; return 1; }
  read -r w h < <(identify -format '%w %h\n' "${srcs[0]}")
  [ -n "$h" ] && [ "$w" = "$h" ] || { echo "thread $thread painted ${w}x${h}, not a square" >&2; return 1; }
  magick "${srcs[0]}" -resize "${size}x${size}" -strip png:- | pngquant --quality 70-95 --speed 1 - > "$target.part" || { echo "thread $thread: resize or quantise failed" >&2; return 1; }
  mv "$target.part" "$target"
}

mkdir -p "$(dirname "$target")"
rm -f "$target"
attempts=4
for attempt in $(seq 1 "$attempts"); do
  rm -f "$target.part"
  one && break
  if [ "$attempt" -eq "$attempts" ]; then
    echo "$target: gave up after $attempts attempts" >&2
    exit 1
  fi
  wait=$((1 << (attempt - 1)))
  echo "$target: attempt $attempt of $attempts failed, retrying in ${wait}s" >&2
  sleep "$wait"
done
[ -s "$target" ] || { echo "$target: empty" >&2; exit 1; }
