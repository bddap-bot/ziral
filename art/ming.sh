#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: ming.sh design OUT.png CAPTION.txt | ming.sh design-layer OUTDIR IMAGE.png PLAN.txt" >&2
  exit 2
}
[ $# -ge 3 ] || usage
: "${OPENROUTER_API_KEY:?OPENROUTER_API_KEY is unset}"
api=https://openrouter.ai/api/v1
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT

get() {
  curl -sS --fail --max-time 60 -H "Authorization: Bearer $OPENROUTER_API_KEY" "$api/$1"
}

usage_now() {
  get credits | jq -e '.data.total_usage'
}

free() {
  get "images/models/inclusionai/ming-image-0.1-$1/endpoints" |
    jq -e '[.endpoints[].pricing[].cost_usd] | length > 0 and all(. == 0)' >/dev/null
}

post() {
  local body=$1 out=$2 code attempt
  for attempt in 1 2 3 4; do
    code=$(curl -sS --max-time 900 -o "$out" -w '%{http_code}' -X POST "$api/images" \
      -H "Authorization: Bearer $OPENROUTER_API_KEY" -H "Content-Type: application/json" \
      --data-binary @"$body") || code=000
    if [ "$code" = 200 ] && jq -e '.data | length > 0' "$out" >/dev/null 2>&1; then
      return 0
    fi
    echo "ming: HTTP $code on attempt $attempt: $(head -c 300 "$out" 2>/dev/null)" >&2
    sleep $((10 << (attempt - 1)))
  done
  return 1
}

model=$1
free "$model" || { echo "ming: $model is no longer listed at \$0" >&2; exit 3; }
before=$(usage_now)
started=$(date +%s.%N)
case $model in
design)
  [ $# -eq 3 ] || usage
  jq -n --rawfile prompt "$3" \
    '{model: "inclusionai/ming-image-0.1-design", prompt: ($prompt | rtrimstr("\n")), output_format: "png", n: 1}' \
    > "$scratch/body.json"
  post "$scratch/body.json" "$scratch/reply.json"
  jq -r '.data[0].b64_json' "$scratch/reply.json" | base64 -d > "$2.part"
  mv "$2.part" "$2"
  ;;
design-layer)
  [ $# -eq 4 ] || usage
  mkdir -p "$2"
  base64 -w0 "$3" > "$scratch/image.b64"
  jq -n --rawfile prompt "$4" --rawfile image "$scratch/image.b64" \
    '{model: "inclusionai/ming-image-0.1-design-layer", prompt: ($prompt | rtrimstr("\n")), output_format: "png", n: 1,
      input_references: [{type: "image_url", image_url: {url: ("data:image/png;base64," + $image)}}]}' \
    > "$scratch/body.json"
  post "$scratch/body.json" "$scratch/reply.json"
  count=$(jq '.data | length' "$scratch/reply.json")
  for i in $(seq 1 "$count"); do
    jq -r ".data[$((i - 1))].b64_json" "$scratch/reply.json" | base64 -d > "$2/layer-$i.png"
  done
  ;;
*) usage ;;
esac
seconds=$(echo "$(date +%s.%N) - $started" | bc)
cost=$(jq -e '.usage.cost // 0' "$scratch/reply.json")
after=$(usage_now)
printf '%s\t%.1f\t%s\t%s\t%s\n' "$model" "$seconds" "$cost" "$before" "$after"
if [ "$cost" != 0 ] || [ "$before" != "$after" ]; then
  echo "ming: the call cost $cost; account usage went from $before to $after" >&2
  exit 3
fi
