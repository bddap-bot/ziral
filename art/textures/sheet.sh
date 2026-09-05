#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
cd "$here"
magick montage arm.png source.png bonder.png second-bond.png output.png atom-base.png bond-single.png bond-double.png tile-*.png \
  -tile 8x4 -geometry 192x192+4+4 -background '#6B4F3A' sheet.png
