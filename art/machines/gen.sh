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
  local name=$1 keep=$2 dir=art/machines/$1 count prompt size i from relight edits=()
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
  flock "$here" "$ziral" --keep "$name" "$keep"
  pngquant --quality 70-95 --speed 1 --force --output "$dir/albedo.png" "$dir/albedo.png"
  for _ in 1 2 3; do
    edits=()
    for from in $(printf '%s\n' "$plan" | awk -F'\t' '$1 == "relight" { print $2 }'); do
      IFS=$'\t' read -r _ _ relight < <(row relight "$from")
      art/paint.sh -s "$size" -i "$dir/relit/master.png" "$dir/relit/$from.png" "$relight"
      edits+=("$dir/relit/$from.png")
    done
    if "$ziral" --normals "$name" "${edits[@]}" > "$dir/relit/lights.txt"; then
      cat "$dir/relit/lights.txt"
      return 0
    fi
    cat "$dir/relit/lights.txt"
  done
  return 1
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
while IFS=$'\t' read -r png outside seat palette _ verdict; do
  name=$(basename "$(dirname "$(dirname "$png")")")
  kept=$(row machine "$name" | cut -f4)
  label="$(basename "$png" .png)  out $outside  seat $seat  pal $palette  $verdict"
  [ "$(basename "$png" .png)" != "$name-$kept" ] || label="KEPT $label"
  printf '%s\t%s\n' "$label" "$png"
done < <(cat /dev/null "$here"/*/scores.tsv) | art/sheet.sh 4 "$here/sheet.png" 256
exit "$status"
