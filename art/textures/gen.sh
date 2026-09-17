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
mkdir -p "$out"
for i in $(seq 0 $((count - 1))); do
  stem=$name
  [ "$count" -eq 1 ] || stem=$(printf '%s-%02d' "$name" "$i")
  prompt="$out/$stem.prompt.txt"
  if [ ! -e "$prompt" ]; then
    cp "$here/$name.prompt.txt" "$prompt"
  fi
  "$here/../paint.sh" "${paint[@]}" "$out/$stem.png" "$prompt"
done
