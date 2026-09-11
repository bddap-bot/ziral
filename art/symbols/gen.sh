#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
scratch="$here/.gen"
names=(f r a d q e x shift-w shift-e shift-f shift-c shift-x shift-a)
inputs=()
thumbs=()
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch"
magick -background none "$here/source.svg" "$scratch/source.png"
read -r width height < <(magick identify -format '%w %h\n' "$scratch/source.png")
[ "$width" = 6656 ] && [ "$height" = 512 ] || { echo "source rendered ${width}x${height}, not 6656x512" >&2; exit 1; }
for i in "${!names[@]}"; do
  magick "$scratch/source.png" -crop "512x512+$((i * 512))+0" +repage -depth 8 -strip "$here/${names[$i]}.png"
  inputs+=("$here/${names[$i]}.png")
  magick "$here/${names[$i]}.png" -resize 26x26 "$scratch/${names[$i]}.png"
  thumbs+=("$scratch/${names[$i]}.png")
done
magick "${inputs[@]:0:7}" +append "$scratch/sheet-top.png"
magick "${inputs[@]:7:6}" +append -background '#6B4F3A' -gravity west -extent 3584x512 "$scratch/sheet-bottom.png"
magick "$scratch/sheet-top.png" "$scratch/sheet-bottom.png" -append "$here/sheet.png"
mkdir -p "$here/../../proofs"
magick "${thumbs[@]:0:7}" +append "$scratch/proof-top.png"
magick "${thumbs[@]:7:6}" +append -background none -gravity west -extent 182x26 "$scratch/proof-bottom.png"
magick "$scratch/proof-top.png" "$scratch/proof-bottom.png" -append "$here/../../proofs/tape-case-73.png"
