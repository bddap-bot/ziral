#!/usr/bin/env bash
set -euo pipefail

[ $# -ge 4 ] || { echo "usage: reference.sh OUT.png SIDE LAYOUT.png REFERENCE.png < PROMPT" >&2; exit 2; }
cat >/dev/null
cp "$4" "$1"
echo 0
