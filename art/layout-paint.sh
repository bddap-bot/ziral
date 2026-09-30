#!/usr/bin/env bash
set -euo pipefail
[ "$#" -eq 2 ] || exit 2
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
layout=$(dirname "$2")/layout.png
prompt=$(dirname "$2")/prompt.txt
cat "$1" > "$prompt"
cat >> "$prompt" <<'PROMPT'

Use case: sketch-to-render. The attached image is a strict layout scaffold, not a style reference. Keep its square canvas and the position and scale of its footprint and circular seats exactly unchanged. Paint a single substantial machine filling that footprint with generous ornamental hardware: riveted service plates, ports, toothed brass collars, pooled ceramic glaze and fine crazing. Arms must visibly read as a pivot joined by a stout forged link to an open gripper, not separate hexagonal tiles. Fuse adjacent body cells into one sculpted housing. The rings in the scaffold locate the pivot, hand and atom apertures: keep their centres exact. Keep circular aperture interiors and all space outside the footprint genuinely transparent, not white or a painted checkerboard. No extra holes, text, labels, axes, perspective or cast shadow. Broad filled bodies with rich depth and materials under diffuse light. Return exactly one square RGBA game sprite.
PROMPT
"$here/paint.sh" -i "$layout" "$2" "$prompt"
