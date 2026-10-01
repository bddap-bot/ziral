#!/usr/bin/env bash
set -euo pipefail
trap 'exit 3' ERR
[ "$#" -eq 2 ] || exit 2
: "${OPENROUTER_API_KEY:?OPENROUTER_API_KEY is unset}"
dir=$(dirname "$2")
layout=$dir/layout.png
prompt=$dir/prompt.txt
scratch=$(mktemp -d "$dir/.gemini.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
cat "$1" > "$prompt"
{
cat <<'LAYOUT'

Use case: sketch-to-render. The attached image is a strict layout scaffold, not a style reference. Keep its square canvas and the position and scale of its footprint and circular seats exactly unchanged. Paint a single substantial machine filling that footprint with generous ornamental hardware: riveted service plates, ports, toothed brass collars, pooled ceramic glaze and fine crazing. Arms must visibly read as a pivot joined by a stout forged link to an open gripper, not separate hexagonal tiles. Fuse adjacent body cells into one sculpted housing. The rings in the scaffold locate the pivot, hand and atom apertures: keep their centres exact. Keep circular aperture interiors and all space outside the footprint genuinely transparent, not white or a painted checkerboard. No extra holes, text, labels, axes, perspective or cast shadow. Broad filled bodies with rich depth and materials under diffuse light. Return exactly one square RGBA game sprite.
LAYOUT
printf '\nImage inputs:\nsha256 %s "%s"\n' "$(sha256sum "$layout" | cut -d' ' -f1)" "$layout"
cat <<'PROMPT'

Key-colour override: Paint EVERY circular seat interior, including the left pivot seat and right hand seat of the arm, as flat uniform RGB (255, 0, 255), hex #FF00FF. Use this colour nowhere else in the painting. These are open seats, never closed hubs. No checkerboard, shading, texture or gradient inside any seat. The adapter will turn this key colour into full transparency. All other caption and layout requirements remain unchanged.
PROMPT
} >> "$prompt"
base64 -w0 "$layout" > "$scratch/layout.b64"
jq -n --rawfile prompt "$prompt" --rawfile image "$scratch/layout.b64" \
  '{model:"google/gemini-3-pro-image",prompt:$prompt,aspect_ratio:"1:1",n:1,output_format:"png",resolution:"1K",input_references:[{type:"image_url",image_url:{url:("data:image/png;base64,"+$image)}}]}' > "$scratch/request.json"
start=$SECONDS
if ! curl -sS --fail --max-time 600 https://openrouter.ai/api/v1/images \
  -H "Authorization: Bearer $OPENROUTER_API_KEY" -H 'Content-Type: application/json' \
  --data-binary @"$scratch/request.json" -o "$scratch/response.json"; then
  echo 'gemini-key: request failed; stopping without paid retry' >&2
  exit 3
fi
jq 'del(.data)' "$scratch/response.json" > "$dir/response-metadata.json"
jq -en --argjson response "$(cat "$dir/response-metadata.json")" \
  '$response.usage.cost | type == "number"' >/dev/null || exit 3
printf 'gemini-key\t%ss\t$%s\n' "$((SECONDS-start))" "$(jq -r '.usage.cost' "$dir/response-metadata.json")"
jq -er '.data[0].b64_json' "$scratch/response.json" | base64 -d > "$dir/returned.png" || exit 3
read -r width height < <(magick identify -format '%w %h\n' "$dir/returned.png")
if [ "$width" != "$height" ] || [ "$width" -eq 0 ]; then
  echo 'gemini-key: nonsquare return; stopping without paid retry' >&2
  exit 3
fi
magick "$layout" -resize "${width}x${height}!" -alpha extract -threshold 50% \
  -fill 'gray(50%)' -draw 'color 0,0 floodfill' -fx 'u==0.5 || (u>0.49 && u<0.51) ? 0 : 1' "$scratch/exterior.png"
magick "$dir/returned.png" -alpha on -channel A \
  -fx 'max(max(abs(r*255-255),g*255),abs(b*255-255))<=16 ? 0 : a' \
  +channel "$scratch/keyed.png"
magick "$scratch/keyed.png" -alpha extract "$scratch/exterior.png" -compose Multiply -composite "$scratch/alpha.png"
magick "$dir/returned.png" "$scratch/alpha.png" -alpha off -compose CopyOpacity -composite "PNG32:$2"
