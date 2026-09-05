#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
name=$1
subject=$2
lock='Ethos: Fired Workshop treats the board as a tabletop instrument assembled from glazed ceramic, darkened brass, and soft rubber. Weight, wear, and warm raking light make each action tactile while colored glazes keep states unmistakable. Palette and roles: board #D8C3A5 clay; arm #6B4F3A dark brass; closed hand #C8553D terracotta; atom kinds #4F8A8B blue-green and #E0A458 amber; glyph #7D5BA6 plum; product #F4EDE4 ivory.'
prompt="Generate exactly one square image with the image generation tool, then stop: do not judge, retry, edit, or describe it, and write no files. Prompt: $subject $lock No labels, text, watermark, menus, clutter, or named-game resemblance."

events=$(codex exec --skip-git-repo-check --json "$prompt" </dev/null)
thread=$(printf '%s\n' "$events" | jq -r 'select(.type == "thread.started") | .thread_id' | head -1)
[ -n "$thread" ] || { printf '%s\n' "$events" >&2; exit 1; }
srcs=("$HOME/.codex/generated_images/$thread"/*.png)
[ "${#srcs[@]}" -eq 1 ] || { echo "thread $thread holds ${#srcs[@]} images, not one" >&2; exit 1; }
magick "${srcs[0]}" -resize 1024x1024! -strip png:- | pngquant --quality 70-95 --speed 1 - > "$here/$name.png"
printf '%s — candidate 1\n%s\n\n' "$name.png" "$subject" >> "$here/prompts.txt"
