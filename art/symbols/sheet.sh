#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
cd "$here"
args=()
for png in [a-z].png; do
  args+=(-label "${png%.png}" "$png")
done
magick montage "${args[@]}" -tile x1 -geometry 256x256+8+8 -background '#6B4F3A' -fill '#F4EDE4' -font "${FONT:-DejaVu-Sans}" -pointsize 28 png:- | pngquant --quality 70-95 - > sheet.png
