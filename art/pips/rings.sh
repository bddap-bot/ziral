#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(dirname "$(dirname "$here")")
scratch=$(mktemp -d "$here/.rings.XXXXXX")
target="$root/proofs/concentric-rings-113.png"
trap 'rm -rf "$scratch"' EXIT

cargo run -- --shot "$scratch/after.png" exponential-pips-83 1
magick "$root/proofs/exponential-pips-83.png" -crop 640x720+640+0 +repage "$scratch/before.png"
magick "$scratch/after.png" -crop 640x720+0+0 +repage "$scratch/after-crop.png"
magick "$scratch/before.png" "$scratch/after-crop.png" +append -depth 8 -strip "$target"
read -r width height < <(magick identify -format '%w %h\n' "$target")
[ "$width" = 1280 ] && [ "$height" = 720 ] || { echo "proof is ${width}x${height}, not 1280x720" >&2; exit 1; }
