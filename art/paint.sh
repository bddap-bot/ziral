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
lock='Ethos: Fired Workshop treats the board as a tabletop instrument assembled from glazed ceramic, darkened brass, and soft rubber. Weight, wear, and warm raking light make each action tactile while colored glazes keep states unmistakable. Palette and roles: board #D8C3A5 clay; arm #6B4F3A dark brass; closed hand #C8553D terracotta; atom kinds #4F8A8B blue-green and #E0A458 amber; glyph #7D5BA6 plum; product #F4EDE4 ivory.'
prompt="Generate exactly one square image with the image generation tool, then stop: do not judge, retry, edit, or describe it, and write no files. Prompt: $subject $lock No labels, watermark, menus, clutter, or named-game resemblance."

one() {
  local events thread srcs w h
  events=$(codex exec --skip-git-repo-check --json "$prompt" "${attach[@]}" </dev/null)
  thread=$(printf '%s\n' "$events" | jq -r 'select(.type == "thread.started") | .thread_id' | head -1)
  [ -n "$thread" ] || { printf '%s\n' "$events" >&2; return 1; }
  srcs=("$HOME/.codex/generated_images/$thread"/*.png)
  [ "${#srcs[@]}" -eq 1 ] || { echo "thread $thread holds ${#srcs[@]} images, not one" >&2; return 1; }
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
