#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
cd "$here/../.."
cargo build --quiet
ziral=${CARGO_TARGET_DIR:-target}/debug/ziral
usage='usage: gen.sh all | gen.sh NAME... | gen.sh -k INDEX NAME'

keep=
while getopts 'k:' opt; do
  case $opt in
  k) keep=$OPTARG ;;
  *) echo "$usage" >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
[ $# -gt 0 ] || { echo "$usage" >&2; exit 2; }
if [ -n "$keep" ] && { [ $# -ne 1 ] || [ "$1" = all ]; }; then
  echo "$usage" >&2
  exit 2
fi

plan=$("$ziral" --plan)
row() { printf '%s\n' "$plan" | awk -F'\t' -v k="$1" -v n="$2" '$1 == k && $2 == n'; }

machine() {
  local name=$1 keep=$2 dir=art/machines/$1 count prompt size i
  IFS=$'\t' read -r _ _ count _ prompt < <(row machine "$name")
  size=$("$ziral" --scaffold "$name")
  if [ -z "$keep" ]; then
    rm -rf "$dir/candidates"
    for i in $(seq 1 "$count"); do
      art/paint.sh -s "$size" -i "$dir/scaffold.png" "$dir/candidates/$name-$i.png" "$prompt"
    done
    "$ziral" --score "$name" "$dir"/candidates/*.png > "$dir/scores.tsv"
    keep=$(sort -t$'\t' -k6,6r -k5,5gr "$dir/scores.tsv" | awk -F'\t' 'NR == 1 && $6 == "pass" { print $1 }' | sed 's/.*-\([0-9]*\)\.png$/\1/')
    [ -n "$keep" ] || { echo "$name: no candidate passes; see $dir/scores.tsv" >&2; return 1; }
  fi
  flock "$here/manifest.toml" "$ziral" --keep "$name" "$keep"
}

status=0
if [ "$1" = all ]; then
  pids=()
  for name in $(printf '%s\n' "$plan" | awk -F'\t' '$1 == "machine" { print $2 }'); do
    mkdir -p "$here/$name"
    machine "$name" "" > "$here/$name/gen.log" 2>&1 &
    pids+=($!)
  done
  for pid in "${pids[@]}"; do wait "$pid" || status=1; done
  cat /dev/null "$here"/*/gen.log
else
  for name in "$@"; do machine "$name" "$keep" || status=1; done
fi

plan=$("$ziral" --plan)
args=()
while IFS=$'\t' read -r png outside seat palette _ verdict; do
  name=$(basename "$(dirname "$(dirname "$png")")")
  kept=$(row machine "$name" | cut -f4)
  label="$(basename "$png" .png)  out $outside  seat $seat  pal $palette  $verdict"
  [ "$(basename "$png" .png)" != "$name-$kept" ] || label="KEPT $label"
  args+=(-label "$label" "$png")
done < <(cat /dev/null "$here"/*/scores.tsv)
part=$(mktemp "$here/sheet.XXXXXX")
trap 'rm -f "$part"' EXIT
magick montage "${args[@]}" -tile 4x -geometry 256x256+6+6 -background '#6B4F3A' -fill '#F4EDE4' -font "${FONT:-DejaVu-Sans}" -pointsize 14 png:- | pngquant --quality 70-95 - > "$part"
mv "$part" "$here/sheet.png"
exit "$status"
