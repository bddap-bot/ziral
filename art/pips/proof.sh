#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(dirname "$(dirname "$here")")
scratch=$(mktemp -d "$here/.proof.XXXXXX")
target="$root/proofs/pip-proposals-84.png"
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch"

magick "$root/proofs/exponential-pips-83.png" -crop 320x720+640+0 +repage "$scratch/board.png"
magick "$scratch/board.png" "$scratch/board.png" "$scratch/board.png" "$scratch/board.png" +append "$scratch/background.png"
magick -background none "$here/source.svg" "$scratch/overlay.png"
read -r width height < <(magick identify -format '%w %h\n' "$scratch/overlay.png")
[ "$width" = 1280 ] && [ "$height" = 720 ] || { echo "source rendered ${width}x${height}, not 1280x720" >&2; exit 1; }
magick "$scratch/background.png" "$scratch/overlay.png" -compose over -composite -depth 8 -strip "$target"
