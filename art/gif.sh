#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
[ $# -ge 4 ] || { echo "usage: gif.sh OUT.gif SCENE TICKS CROP(W:H:X:Y) [TICK_MS] [FPS]" >&2; exit 2; }
out=$1
scene=$2
ticks=$3
crop=$4
tick_ms=${5:-400}
fps=${6:-20}
frames=$(mktemp -d)
trap 'rm -rf "$frames" "$out.part.gif"' EXIT
cd "$here/.."
cargo run -- --shot "$frames" "$scene" 0 "$ticks" "$tick_ms" 1
ffmpeg -loglevel error -y -framerate 60 -i "$frames/%05d.png" -vf "crop=$crop,fps=$fps,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3" -loop 0 "$out.part.gif"
mv "$out.part.gif" "$out"
