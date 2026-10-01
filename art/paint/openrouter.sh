#!/usr/bin/env bash
set -euo pipefail

[ $# -ge 3 ] || { echo "usage: openrouter.sh MODEL OUT.png SIDE [IMAGE]... < PROMPT" >&2; exit 2; }
model=$1 out=$2 side=$3
shift 3
api=https://openrouter.ai/api/v1
auth="Authorization: Bearer ${OPENROUTER_API_KEY:?}"
request=$(mktemp)
reply=$(mktemp)
trap 'rm -f "$request" "$reply"' EXIT
tier=$(curl -sS --max-time 60 --fail "$api/images/models" -H "$auth" | jq -r --arg model "$model" --argjson side "$side" '
  [.data[] | select(.id == $model) | .supported_parameters.resolution.values // [] | .[]
    | {tier: ., px: (if endswith("K") then (.[:-1] | tonumber) * 1024 else tonumber end)}]
  | ((map(select(.px >= $side)) | min_by(.px)) // max_by(.px)) | .tier // empty')
for image in "$@"; do
  base64 -w0 "$image" | jq -Rc '{type: "image_url", image_url: {url: ("data:image/png;base64," + .)}}'
done >"$reply"
jq -n --arg model "$model" --rawfile prompt /dev/stdin --arg tier "$tier" --slurpfile references "$reply" '
  {model: $model, prompt: $prompt, aspect_ratio: "1:1", input_references: $references}
  + if $tier == "" then {} else {resolution: $tier} end' >"$request"
curl -sS --max-time 600 --fail-with-body "$api/images" -H "$auth" -H 'Content-Type: application/json' \
  --data-binary @"$request" >"$reply" || { head -c 2000 "$reply" >&2; exit 1; }
jq -r '.usage.cost' "$reply"
jq -r '.data[0].b64_json // empty' "$reply" | base64 -d | magick - "png:$out" || { jq -c 'del(.data)' "$reply" | head -c 2000 >&2; exit 1; }
