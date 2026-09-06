#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
shot=$(mktemp -d)
trap 'rm -rf "$shot" "$here"/proof/tab-*.png.part' EXIT
cd "$here/../.."
mkdir -p "$here/proof"
for scene in tab-held tab-released; do
  cargo run -- --shot "$shot/$scene.png" "$scene" 8
  pngquant --quality 70-95 "$shot/$scene.png" --output "$here/proof/$scene.png.part"
  mv "$here/proof/$scene.png.part" "$here/proof/$scene.png"
done
