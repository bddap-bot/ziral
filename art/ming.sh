#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: ming.sh design OUT.png CAPTION.txt | ming.sh design-layer OUTDIR IMAGE.png PLAN.txt" >&2
  exit 2
}
case ${1:-} in
design) [ $# -eq 3 ] || usage ;;
design-layer) [ $# -eq 4 ] || usage ;;
*) usage ;;
esac
: "${OPENROUTER_API_KEY:?OPENROUTER_API_KEY is unset}"
model=$1
api=https://openrouter.ai/api/v1
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT

get() {
  curl -sS --fail --max-time 60 -H "Authorization: Bearer $OPENROUTER_API_KEY" "$api/$1"
}

usage_now() {
  get credits | jq -e '.data.total_usage'
}

spent() {
  local after
  after=$(usage_now)
  if ! jq -en --argjson a "$before" --argjson b "$after" '$a == $b' >/dev/null; then
    echo "ming: account usage went from $before to $after" >&2
    exit 3
  fi
}

post() {
  local code attempt
  for attempt in 1 2 3 4; do
    [ "$attempt" -eq 1 ] || sleep $((10 << (attempt - 2)))
    code=$(curl -sS --max-time 900 -o "$scratch/reply.json" -w '%{http_code}' -X POST "$api/images" \
      -H "Authorization: Bearer $OPENROUTER_API_KEY" -H "Content-Type: application/json" \
      --data-binary @"$scratch/body.json") || code=000
    if [ "$code" = 200 ] && jq -e '.data | length > 0' "$scratch/reply.json" >/dev/null 2>&1; then
      return 0
    fi
    echo "ming: HTTP $code on attempt $attempt: $(head -c 300 "$scratch/reply.json" 2>/dev/null)" >&2
  done
  spent
  exit 1
}

pricing=$(get "images/models/inclusionai/ming-image-0.1-$model/endpoints")
if ! jq -e '[.endpoints[].pricing[].cost_usd] | length > 0 and all(. == 0)' <<<"$pricing" >/dev/null; then
  echo "ming: $model is no longer listed at \$0" >&2
  exit 3
fi
before=$(usage_now)
started=$(date +%s.%N)
case $model in
design)
  jq -n --rawfile prompt "$3" \
    '{model: "inclusionai/ming-image-0.1-design", prompt: ($prompt | rtrimstr("\n")), output_format: "png", n: 1}' \
    > "$scratch/body.json"
  post
  jq -r '.data[0].b64_json' "$scratch/reply.json" | base64 -d > "$2.part"
  mv "$2.part" "$2"
  ;;
design-layer)
  mkdir -p "$2"
  base64 -w0 "$3" > "$scratch/image.b64"
  jq -n --rawfile prompt "$4" --rawfile image "$scratch/image.b64" \
    '{model: "inclusionai/ming-image-0.1-design-layer", prompt: ($prompt | rtrimstr("\n")), output_format: "png", n: 1,
      input_references: [{type: "image_url", image_url: {url: ("data:image/png;base64," + $image)}}]}' \
    > "$scratch/body.json"
  post
  count=$(jq '.data | length' "$scratch/reply.json")
  for i in $(seq 1 "$count"); do
    jq -r ".data[$((i - 1))].b64_json" "$scratch/reply.json" | base64 -d > "$2/layer-$i.png"
  done
  ;;
esac
seconds=$(echo "$(date +%s.%N) - $started" | bc)
cost=$(jq '.usage.cost // 0' "$scratch/reply.json")
printf '%s\t%.1f\t%s\n' "$model" "$seconds" "$cost"
if ! jq -en --argjson c "$cost" '$c == 0' >/dev/null; then
  echo "ming: the call cost $cost" >&2
  exit 3
fi
spent
