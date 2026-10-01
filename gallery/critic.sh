#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
[ "$#" -eq 1 ] || { echo 'usage: gallery/critic.sh GALLERY_DIRECTORY' >&2; exit 2; }
node gallery/critic.mjs "$1"
