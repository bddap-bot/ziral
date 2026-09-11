#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
cd "$here"
slots=('f 96 64' 'r 544 64' 'a 96 200' 'd 544 200' 'q 96 336' 'e 544 336' 'x 96 472' 'shift-w 96 608' 'shift-e 544 608' 'shift-f 96 744' 'shift-c 544 744' 'shift-x 96 880' 'shift-a 544 880')
inputs=(page.png)
args=(page.png)
trap 'rm -f manual.png.part inputs.sha256.part' EXIT
if [ $# -gt 1 ] || { [ $# -eq 1 ] && [ "$1" != -c ]; }; then
  echo "usage: gen.sh [-c]" >&2
  exit 2
fi
read -r width height < <(magick identify -format '%w %h\n' page.png)
[ "$width" = 1024 ] && [ "$height" = 1024 ] || { echo "page rendered ${width}x${height}, not 1024 square" >&2; exit 1; }
for row in "${slots[@]}"; do
  read -r key x y <<< "$row"
  inputs+=("../symbols/$key.png")
  args+=(\( "../symbols/$key.png" -resize 128x128 \) -geometry "+$x+$y" -composite)
done
magick "${args[@]}" -depth 8 -strip manual.png.part
if [ $# -eq 1 ]; then
  sha256sum --check --quiet --strict inputs.sha256
  cmp --silent manual.png.part manual.png || { echo "manual.png is stale" >&2; exit 1; }
  exit 0
fi
mv manual.png.part manual.png
sha256sum "${inputs[@]}" > inputs.sha256.part
mv inputs.sha256.part inputs.sha256
