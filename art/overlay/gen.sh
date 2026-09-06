#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
cd "$here"
trap 'rm -f scaffold.png.part manual.png.part inputs.sha256.part proof/slots.png.part' EXIT
slot=128
slots=('f 96 128' 'r 544 128' 'a 96 352' 'd 544 352' 'q 96 576' 'e 544 576' 'x 96 800')
inputs=(prompt.txt scaffold.png)

scaffold() {
  local args=(-size 1024x1024 "xc:#D8C3A5") row key x y
  for row in "${slots[@]}"; do
    read -r key x y <<< "$row"
    args+=(\( "../symbols/$key.png" -resize "${slot}x${slot}" \) -geometry "+$x+$y" -composite)
  done
  magick "${args[@]}" -depth 8 -strip png:scaffold.png.part
  mv scaffold.png.part scaffold.png
}
count=1
keep=
check=
mode=
while getopts 'n:k:c' opt; do
  case $opt in
  n) count=$OPTARG ;;
  k) keep=$OPTARG ;;
  c) check=1 ;;
  *) mode='??' ;;
  esac
  mode+=$opt
done
if [ ${#mode} -gt 1 ]; then
  echo "usage: gen.sh [-n COUNT] | gen.sh -k INDEX | gen.sh -c" >&2
  exit 2
fi

if [ -n "$check" ]; then
  scaffold
  exec sha256sum --check --quiet --strict inputs.sha256
fi

if [ -z "$keep" ]; then
  scaffold
  read -r -d '' subject < prompt.txt || true
  ../textures/gen.sh -o "$here" -n "$count" -i "$here/scaffold.png" manual "$subject"
  if [ "$count" -gt 1 ]; then
    exit 0
  fi
else
  cp "candidates/manual-$keep.png" manual.png.part
  mv manual.png.part manual.png
  printf 'kept manual-%s\n\n' "$keep" >> prompts.txt
fi
sha256sum "${inputs[@]}" > inputs.sha256.part
mv inputs.sha256.part inputs.sha256

args=()
for row in "${slots[@]}"; do
  read -r key x y <<< "$row"
  args+=(\( "../symbols/$key.png" -resize "${slot}x${slot}" \) \( manual.png -crop "${slot}x${slot}+$x+$y" +repage \))
done
mkdir -p proof
magick montage "${args[@]}" -font "${FONT:-DejaVu-Sans}" -tile 2x -geometry +4+4 -background '#6B4F3A' png:- | pngquant --quality 70-95 - > proof/slots.png.part
mv proof/slots.png.part proof/slots.png
