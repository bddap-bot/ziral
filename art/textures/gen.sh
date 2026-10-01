#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
out=$here
count=1
size=1024
images=()
generator=$(cat "$here/../paint/default")
usage() { echo "usage: gen.sh [-o DIR] [-n COUNT] [-s SIZE] [-i IMAGE] NAME" >&2; exit 2; }
while getopts 'o:n:s:i:' opt; do
  case $opt in
  o) out=$OPTARG ;;
  n) count=$OPTARG ;;
  s) size=$OPTARG ;;
  i) images+=("$(realpath "$OPTARG")") ;;
  *) usage ;;
  esac
done
shift $((OPTIND - 1))
[ $# -eq 1 ] || usage
name=$1
mkdir -p "$out"
out=$(realpath "$out")
cd "$here/../.."
for i in $(seq 0 $((count - 1))); do
  stem=$name
  [ "$count" -eq 1 ] || stem=$(printf '%s-%02d' "$name" "$i")
  prompt="$out/$stem.prompt.txt"
  if [ ! -e "$prompt" ]; then
    cp "$here/$name.prompt.txt" "$prompt"
  fi
  sh -c "$generator \"\$@\"" paint "$out/$stem.png.part" "$size" "${images[@]}" <"$prompt" >/dev/null
  [ "$(magick identify -format '%w' "$out/$stem.png.part")" = "$(magick identify -format '%h' "$out/$stem.png.part")" ] || { echo "$stem: off-aspect return" >&2; exit 1; }
  magick "$out/$stem.png.part" -resize "${size}x${size}" -strip png:- | pngquant --quality 70-95 --speed 1 - >"$out/$stem.png"
  rm "$out/$stem.png.part"
done
