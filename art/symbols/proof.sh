#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
shot=$(mktemp -d)
trap 'rm -rf "$shot" "$here/proof/tape-26px.png.part"' EXIT
cd "$here/../.."
cargo run -- --shot "$shot/focus.png" focus 8
magick "$shot/focus.png" -crop 330x98+140+590 +repage "$shot/crop.png"
magick "$shot/crop.png" -filter point -resize 400% "$shot/big.png"
mkdir -p "$here/proof"
magick "$shot/crop.png" "$shot/big.png" -background '#6B4F3A' -gravity west -append png:- | pngquant --quality 70-95 - > "$here/proof/tape-26px.png.part"
mv "$here/proof/tape-26px.png.part" "$here/proof/tape-26px.png"
