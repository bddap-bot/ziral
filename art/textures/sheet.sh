#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
cd "$here"
rows=()
row() {
  local out="$1"
  shift
  magick "$@" -resize 192x192 -bordercolor '#6B4F3A' -border 4 +append "$out"
  rows+=("$out")
}
row /tmp/sheet-row0.png arm.png source.png bonder.png second-bond.png output.png atom-base.png bond-single.png bond-double.png
row /tmp/sheet-row1.png tile-0[0-7].png
row /tmp/sheet-row2.png tile-0[8-9].png tile-1[0-5].png
row /tmp/sheet-row3.png tile-1[6-9].png tile-2[0-3].png
magick "${rows[@]}" -background '#6B4F3A' -append png:- | pngquant --quality 70-95 - > sheet.png
rm -f "${rows[@]}"
