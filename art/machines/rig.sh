#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
for dir in "$here"/*; do
  [ -f "$dir/albedo.png" ] || continue
  mkdir -p "$dir/parts"
  read -r width height < <(magick identify -format '%w %h\n' "$dir/albedo.png")
  radius=$((width < height ? width / 10 : height / 10))
  centre_x=$((width / 2))
  centre_y=$((height / 2))
  edge_x=$((centre_x + radius))
  mask=$(mktemp "$here/mask.XXXXXX.png")
  trap 'rm -f "$mask"' EXIT
  magick -size "${width}x${height}" xc:none -fill white -draw "circle $centre_x,$centre_y $edge_x,$centre_y" "$mask"
  for map in albedo normal; do
    magick "$dir/$map.png" "$mask" -compose DstIn -composite "$dir/parts/${map}-moving.png"
    magick "$dir/$map.png" \( "$mask" -channel A -negate +channel \) -compose DstIn -composite "$dir/parts/${map}-base.png"
  done
  rm -f "$mask"
  trap - EXIT
done
