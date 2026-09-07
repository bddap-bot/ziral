#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
out=$here
count=1
paint=()
while getopts 'o:n:s:i:' opt; do
  case $opt in
  o) out=$OPTARG ;;
  n) count=$OPTARG ;;
  s | i) paint+=("-$opt" "$OPTARG") ;;
  *) echo "usage: gen.sh [-o DIR] [-n COUNT] [-s SIZE] [-i IMAGE] NAME SUBJECT" >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
name=$1
subject=$2

dir=$out
kept="candidate 1"
if [ "$count" -gt 1 ]; then
  dir=$out/candidates
  kept="candidates 1 to $count"
fi
for i in $(seq 1 "$count"); do
  target=$dir/$name.png
  [ "$count" -eq 1 ] || target=$dir/$name-$i.png
  "$here/../paint.sh" "${paint[@]}" "$target" "$subject"
done
printf '%s — %s\n%s\n\n' "$name.png" "$kept" "$subject" >> "$out/prompts.txt"
