#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
shot=$(mktemp -d)
trap 'rm -rf "$shot" "$here"/proof/*.part' EXIT
cd "$here/../.."
cargo run -- --shot "$shot/rotation.png" rotation 8
mkdir -p "$here/proof"
pngquant --quality 70-95 "$shot/rotation.png" --output "$here/proof/rotation.png.part"
magick "$shot/rotation.png" -crop 260x240+370+120 +repage -filter point -resize 200% "$shot/turn0.png"
magick "$shot/rotation.png" -crop 260x240+860+130 +repage -filter point -resize 200% "$shot/turn2.png"
magick "$shot/turn0.png" "$shot/turn2.png" +append png:- | pngquant --quality 70-95 - > "$here/proof/turns.png.part"
mv "$here/proof/rotation.png.part" "$here/proof/rotation.png"
mv "$here/proof/turns.png.part" "$here/proof/turns.png"
