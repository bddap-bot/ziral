#!/usr/bin/env bash
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
attach=()
while getopts 'i:' opt; do
  case $opt in
  i) attach+=(-i "$OPTARG") ;;
  *) echo "usage: direct.sh [-i IMAGE]... BRIEF" >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
[ $# -eq 1 ] || { echo "usage: direct.sh [-i IMAGE]... BRIEF" >&2; exit 2; }
director="You are the art director for one game picture and you write the prompt an image generation model will receive. The brief below states the facts the finished picture must have; the wording of its direction is a starting point you may change or drop as you see fit, and within those facts the art direction is yours: be creative. Write the prompt as a declarative caption describing the finished picture, its subject, layout, materials, light and what is absent, never as instructions to the model. Answer with exactly one JSON object and nothing else, {\"prompt\": the caption as one string}; do not generate an image, run commands, edit anything or write files. Brief:"
schema='{"type":"object","properties":{"prompt":{"type":"string","minLength":1}},"required":["prompt"],"additionalProperties":false}'
"$here/ask.sh" "${attach[@]}" "$schema" "$director $1" | jq -Rrse '(capture("(?<object>\\{.*\\})"; "s").object | fromjson).prompt | select(length > 0)'
