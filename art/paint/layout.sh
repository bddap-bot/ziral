#!/usr/bin/env bash
set -euo pipefail

[ $# -ge 3 ] || { echo "usage: layout.sh OUT.png SIDE LAYOUT.png [REFERENCE.png]... < PROMPT" >&2; exit 2; }
cat >/dev/null
cp "$3" "$1"
echo 0
