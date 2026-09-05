#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
out=$here
count=1
size=1024
while getopts 'o:n:s:' opt; do
  case $opt in
  o) out=$OPTARG ;;
  n) count=$OPTARG ;;
  s) size=$OPTARG ;;
  *) echo "usage: gen.sh [-o DIR] [-n COUNT] [-s SIZE] NAME SUBJECT" >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
name=$1
subject=$2
lock='Ethos: Fired Workshop treats the board as a tabletop instrument assembled from glazed ceramic, darkened brass, and soft rubber. Weight, wear, and warm raking light make each action tactile while colored glazes keep states unmistakable. Palette and roles: board #D8C3A5 clay; arm #6B4F3A dark brass; closed hand #C8553D terracotta; atom kinds #4F8A8B blue-green and #E0A458 amber; glyph #7D5BA6 plum; product #F4EDE4 ivory.'
prompt="Generate exactly one square image with the image generation tool, then stop: do not judge, retry, edit, or describe it, and write no files. Prompt: $subject $lock No labels, watermark, menus, clutter, or named-game resemblance."

one() {
  local target=$1
  local events thread srcs
  events=$(codex exec --skip-git-repo-check --json "$prompt" </dev/null)
  thread=$(printf '%s\n' "$events" | jq -r 'select(.type == "thread.started") | .thread_id' | head -1)
  [ -n "$thread" ] || { printf '%s\n' "$events" >&2; return 1; }
  srcs=("$HOME/.codex/generated_images/$thread"/*.png)
  [ "${#srcs[@]}" -eq 1 ] || { echo "thread $thread holds ${#srcs[@]} images, not one" >&2; return 1; }
  magick "${srcs[0]}" -resize "${size}x${size}!" -strip png:- | pngquant --quality 70-95 --speed 1 - > "$target"
}

if [ "$count" -eq 1 ]; then
  one "$out/$name.png"
  printf '%s — candidate 1\n%s\n\n' "$name.png" "$subject" >> "$out/prompts.txt"
else
  mkdir -p "$out/candidates"
  for i in $(seq 1 "$count"); do
    one "$out/candidates/$name-$i.png"
  done
  printf '%s — candidates 1 to %s\n%s\n\n' "$name.png" "$count" "$subject" >> "$out/prompts.txt"
fi
