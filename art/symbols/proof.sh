#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
scene=${1:-focus}
stem=${2:-tape-26px}
shot=$(mktemp -d)
trap 'rm -rf "$shot" "$here/proof/$stem.png.part"' EXIT
cd "$here/../.."
cargo run -- --shot "$shot/$scene.png" "$scene" 8
magick "$shot/$scene.png" -crop 500x98+82+590 +repage "$shot/crop.png"
magick "$shot/crop.png" -filter point -resize 400% "$shot/big.png"
mkdir -p "$here/proof"
magick "$shot/crop.png" "$shot/big.png" -background '#6B4F3A' -gravity west -append png:- | pngquant --quality 70-95 - > "$here/proof/$stem.png.part"
mv "$here/proof/$stem.png.part" "$here/proof/$stem.png"
