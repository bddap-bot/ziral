#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

size=1024
input=
while getopts 's:i:' opt; do
  case $opt in
  s) size=$OPTARG ;;
  i) input=$(realpath "$OPTARG") ;;
  *) echo "usage: paint.sh [-s SIZE] [-i IMAGE] OUT.png PROMPT" >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
target=$1
subject=$2
lock='Ethos: Fired Workshop treats the board as a tabletop instrument assembled from glazed ceramic, darkened brass, and soft rubber. Weight, wear, and warm raking light make each action tactile while colored glazes keep states unmistakable. Palette and roles: board #D8C3A5 clay; arm #6B4F3A dark brass; closed hand #C8553D terracotta; atom kinds #4F8A8B blue-green and #E0A458 amber; glyph #7D5BA6 plum; product #F4EDE4 ivory.'
prompt="Generate exactly one square image with the image generation tool, then stop: do not judge, retry, edit, or describe it, and write no files. Prompt: $subject $lock No labels, watermark, menus, clutter, or named-game resemblance."

one() {
  local events thread srcs
  local args=(exec --skip-git-repo-check --json "$prompt")
  [ -z "$input" ] || args+=(--image "$input")
  events=$(codex "${args[@]}" </dev/null)
  thread=$(printf '%s\n' "$events" | jq -r 'select(.type == "thread.started") | .thread_id' | head -1)
  [ -n "$thread" ] || { printf '%s\n' "$events" >&2; return 1; }
  srcs=("$HOME/.codex/generated_images/$thread"/*.png)
  [ "${#srcs[@]}" -eq 1 ] || { echo "thread $thread holds ${#srcs[@]} images, not one" >&2; return 1; }
  magick "${srcs[0]}" -resize "${size}x${size}!" -strip png:- | pngquant --quality 70-95 --speed 1 - > "$target.part"
  mv "$target.part" "$target"
}

mkdir -p "$(dirname "$target")"
rm -f "$target"
for _ in 1 2 3 4; do one && break; done
[ -s "$target" ]
