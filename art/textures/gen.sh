#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
out=$here
count=1
paint=()
usage() { echo "usage: gen.sh [-o DIR] [-n COUNT] [-s SIZE] [-i IMAGE] NAME" >&2; exit 2; }
while getopts 'o:n:s:i:' opt; do
  case $opt in
  o) out=$OPTARG ;;
  n) count=$OPTARG ;;
  s | i) paint+=("-$opt" "$OPTARG") ;;
  *) usage ;;
  esac
done
shift $((OPTIND - 1))
[ $# -eq 1 ] || usage
name=$1
subject=$(cat "$here/$name.prompt.txt")
if [ "$count" -eq 1 ]; then
  "$here/../paint.sh" "${paint[@]}" "$out/$name.png" "$subject"
else
  for i in $(seq 0 $((count - 1))); do
    "$here/../paint.sh" "${paint[@]}" "$(printf '%s/%s-%02d.png' "$out" "$name" "$i")" "$subject"
  done
fi
