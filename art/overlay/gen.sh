#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
cd "$here"
slot=128
slots=('f 96 128' 'r 544 128' 'a 96 352' 'd 544 352' 'q 96 576' 'e 544 576' 'x 96 800')
inputs=(../symbols/{f,r,a,d,q,e,x}.png prompt.txt)
count=1
keep=
check=
while getopts 'n:k:c' opt; do
  case $opt in
  n) count=$OPTARG ;;
  k) keep=$OPTARG ;;
  c) check=1 ;;
  *) echo "usage: gen.sh [-n COUNT] | gen.sh -k CANDIDATE | gen.sh -c" >&2; exit 2 ;;
  esac
done

if [ -n "$check" ]; then
  exec sha256sum --check --quiet --strict inputs.sha256
fi

if [ -z "$keep" ]; then
  args=(-size 1024x1024 "xc:#D8C3A5")
  for row in "${slots[@]}"; do
    read -r key x y <<< "$row"
    args+=(\( "../symbols/$key.png" -resize "${slot}x${slot}" \) -geometry "+$x+$y" -composite)
  done
  magick "${args[@]}" -depth 8 -strip png:scaffold.png.part
  mv scaffold.png.part scaffold.png
  read -r -d '' subject < prompt.txt || true
  ../textures/gen.sh -o "$here" -n "$count" -i "$here/scaffold.png" controls "$subject"
  [ "$count" -gt 1 ] && exit 0
  keep=controls.png
else
  keep=candidates/controls-.png
  cp "" controls.png.part
  mv controls.png.part controls.png
fi

args=()
for row in "${slots[@]}"; do
  read -r key x y <<< "$row"
  args+=(\( "../symbols/$key.png" -resize "${slot}x${slot}" \) \( controls.png -crop "${slot}x${slot}+$x+$y" +repage \))
done
mkdir -p proof
magick montage "${args[@]}" -font "${FONT:-DejaVu-Sans}" -tile 2x -geometry +4+4 -background '#6B4F3A' png:- | pngquant --quality 70-95 - > proof/slots.png.part
mv proof/slots.png.part proof/slots.png
sha256sum "${inputs[@]}" > inputs.sha256.part
mv inputs.sha256.part inputs.sha256
printf 'kept %s\n\n' "$keep" >> prompts.txt
