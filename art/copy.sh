#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
[ $# -ge 1 ] || { echo "usage: copy.sh OUT.gif" >&2; exit 2; }
out=$1
ticks=10
copied=190
panel=340
scratch=$(mktemp -d)
Xvfb :99 -screen 0 640x480x24 >/dev/null 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null; rm -rf "$scratch"' EXIT
export DISPLAY=:99
sleep 1
cd "$here/.."
mkdir -p "$scratch/raw" "$scratch/beside"
(
  while sleep 0.1; do
    if text=$(xclip -o -selection clipboard 2>/dev/null) && [ -n "$text" ]; then
      printf '%s\n' "$text" > "$scratch/clip.txt.part"
      mv "$scratch/clip.txt.part" "$scratch/clip.txt"
    fi
  done
) &
poll=$!
cargo run -- --shot "$scratch/raw" copy 0 "$ticks" 400 1
kill $poll
[ -s "$scratch/clip.txt" ] || { echo "the clipboard stayed empty" >&2; exit 1; }
text=$(cat "$scratch/clip.txt")
echo "clipboard: $text"
height=$(magick identify -format '%h' "$scratch/raw/00000.png")
magick -size "${panel}x${height}" xc:'#151311' "$scratch/blank.png"
magick -size "${panel}x${height}" xc:'#151311' -gravity center -fill '#e9dcc3' \
  -font DejaVu-Sans-Mono -pointsize 22 -annotate 0 "$text" "$scratch/text.png"
for png in "$scratch"/raw/*.png; do
  n=$(basename "$png" .png)
  side=$scratch/blank.png
  [ "$((10#$n))" -lt "$copied" ] || side=$scratch/text.png
  magick "$png" "$side" +append -depth 8 "$scratch/beside/$n.png"
done
raw=$(find "$scratch/raw" -name "*.png" | wc -l)
beside=$(find "$scratch/beside" -name "*.png" | wc -l)
echo "frames: $raw raw, $beside beside"
[ "$raw" -eq "$beside" ] || { echo "the composite dropped frames" >&2; exit 1; }
width=$(magick identify -format '%w' "$scratch/beside/00000.png")
FRAMES=$scratch/beside "$here/gif.sh" "$out" copy "$ticks" "$width:620:0:100"
gif=$(magick identify "$out" | wc -l)
echo "frames: $gif in the gif"
[ "$gif" -ge $((raw / 3 - 2)) ] || { echo "the gif dropped frames" >&2; exit 1; }
