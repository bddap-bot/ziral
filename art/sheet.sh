#!/usr/bin/env bash
set -euo pipefail

cd "$1"
cols=$2
args=()
for png in *.png; do
  [ "$png" = sheet.png ] && continue
  args+=(-label "${png%.png}" "$png")
done
magick montage "${args[@]}" -tile "${cols}x" -geometry 192x192+6+6 -background '#6B4F3A' -fill '#F4EDE4' -font "${FONT:-DejaVu-Sans}" -pointsize 24 png:- | pngquant --quality 70-95 - > sheet.png
