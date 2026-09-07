#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

cols=$1
out=$2
size=${3:-192}
args=()
while IFS=$'\t' read -r label png; do
  args+=(-label "$label" "$png")
done
magick montage "${args[@]}" -tile "${cols}x" -geometry "${size}x${size}+6+6" -background '#6B4F3A' -fill '#F4EDE4' -font "${FONT:-DejaVu-Sans}" -pointsize 14 png:- | pngquant --quality 70-95 - > "$out.part"
mv "$out.part" "$out"
