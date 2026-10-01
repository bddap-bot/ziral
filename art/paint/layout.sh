#!/usr/bin/env bash
set -euo pipefail

[ $# -eq 3 ] || { echo "usage: layout.sh OUT.png SIDE LAYOUT.png < PROMPT" >&2; exit 2; }
cat >/dev/null
cp "$3" "$1"
echo 0
