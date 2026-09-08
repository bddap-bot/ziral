#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
out=${1:-$here/reference/proof/cards.png}
shot=$(mktemp -d)
trap 'rm -rf "$shot" "$out.part"' EXIT
cd "$here/.."
cards=()
for name in bonder second-bond arm output-1 output-2 output-3; do
  rect=$(cargo run -- --shot "$shot/$name.png" "card:$name" 0 | awk '$1 == "card" { print $4 "x" $5 "+" $2 "+" $3 }' | head -1)
  [ -n "$rect" ] || { echo "$name: no card rect printed" >&2; exit 1; }
  magick "$shot/$name.png" -crop "$rect" +repage "$shot/card-$name.png"
  cards+=("$shot/card-$name.png")
done
magick "${cards[@]}" -background '#D8C3A5' -gravity west -append png:- | pngquant --quality 70-95 - > "$out.part"
mv "$out.part" "$out"
