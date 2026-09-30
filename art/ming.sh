#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: ming.sh design OUT.png CAPTION.txt | ming.sh design-layer OUTDIR IMAGE.png PLAN.txt" >&2
  exit 2
}
case ${1:-} in
design) [ $# -eq 3 ] || usage; scratch_parent=$(dirname "$2") ;;
design-layer) [ $# -eq 4 ] || usage; scratch_parent=$2 ;;
*) usage ;;
esac
: "${OPENROUTER_API_KEY:?OPENROUTER_API_KEY is unset}"
model=$1
api=https://openrouter.ai/api/v1
mkdir -p "$scratch_parent"
scratch=$(mktemp -d "$scratch_parent/.ming.XXXXXX")
finish() {
  local rc=$?
  if [ "$rc" -eq 3 ] && [ -n "${ZIRAL_GENERATOR_STOP:-}" ]; then
    printf 'zero-spend policy stopped generation\n' > "$ZIRAL_GENERATOR_STOP" || true
  fi
  rm -rf "$scratch"
  exit "$rc"
}
trap finish EXIT

admit() {
  if [ -n "${ZIRAL_GENERATOR_STOP:-}" ] && [ -e "$ZIRAL_GENERATOR_STOP" ]; then
    echo 'ming: another request stopped generation' >&2
    exit 3
  fi
}
admit

get() {
  curl -sS --fail --max-time 60 -H "Authorization: Bearer $OPENROUTER_API_KEY" "$api/$1"
}

credits() {
  local usage
  if ! usage=$(get credits | jq -er '.data.total_usage | select(type == "number")'); then
    echo 'ming: cannot verify account usage; stopping' >&2
    exit 3
  fi
  if ! jq -en --argjson value "$usage" '$value == 0' >/dev/null; then
    echo "ming: account usage is $usage; stopping" >&2
    exit 3
  fi
}

credits
if ! pricing=$(get "images/models/inclusionai/ming-image-0.1-$model/endpoints") ||
  ! jq -e '[.endpoints[].pricing[].cost_usd] | length > 0 and all(. == 0)' <<<"$pricing" >/dev/null; then
  echo "ming: cannot verify $model at \$0; stopping" >&2
  exit 3
fi
case $model in
design)
  jq -n --rawfile prompt "$3" \
    '{model: "inclusionai/ming-image-0.1-design", prompt: ($prompt | rtrimstr("\n")), output_format: "png", n: 1}' \
    > "$scratch/body.json"
  ;;
design-layer)
  base64 -w0 "$3" > "$scratch/image.b64"
  jq -n --rawfile prompt "$4" --rawfile image "$scratch/image.b64" \
    '{model: "inclusionai/ming-image-0.1-design-layer", prompt: ($prompt | rtrimstr("\n")), output_format: "png", n: 1,
      input_references: [{type: "image_url", image_url: {url: ("data:image/png;base64," + $image)}}]}' \
    > "$scratch/body.json"
  ;;
esac
started=$SECONDS
admit
code=$(curl -sS --max-time 900 -o "$scratch/reply.json" -w '%{http_code}' -X POST "$api/images" \
  -H "Authorization: Bearer $OPENROUTER_API_KEY" -H "Content-Type: application/json" \
  --data-binary @"$scratch/body.json") || code=000
credits
cost=$(jq -er 'if (.usage | type) == "object" and (.usage | has("cost")) then .usage.cost else 0 end | select(type == "number")' "$scratch/reply.json" 2>/dev/null) || {
  if jq -e '.usage | has("cost")' "$scratch/reply.json" >/dev/null 2>&1; then
    echo 'ming: invalid reported cost; stopping' >&2
    exit 3
  fi
  cost=0
}
printf '%s\t%s\t%s\n' "$model" "$((SECONDS - started))" "$cost"
if ! jq -en --argjson value "$cost" '$value == 0' >/dev/null; then
  echo "ming: the call cost $cost; stopping" >&2
  exit 3
fi
if [ "$code" != 200 ] || ! jq -e '.data | type == "array" and length > 0' "$scratch/reply.json" >/dev/null; then
  echo "ming: HTTP $code or invalid image response" >&2
  exit 1
fi
case $model in
design)
  jq -er '.data[0].b64_json' "$scratch/reply.json" | base64 -d > "$scratch/design.png"
  mv "$scratch/design.png" "$2"
  ;;
design-layer)
  count=$(jq '.data | length' "$scratch/reply.json")
  for i in $(seq 1 "$count"); do
    jq -er ".data[$((i - 1))].b64_json" "$scratch/reply.json" | base64 -d > "$scratch/layer-$i.png"
  done
  for i in $(seq 1 "$count"); do
    mv "$scratch/layer-$i.png" "$2/layer-$i.png"
  done
  ;;
esac
