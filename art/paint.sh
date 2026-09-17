#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

size=1024
attach=()
images=()
codex_root=${CODEX_HOME:-$HOME/.codex}
root=$(git rev-parse --show-toplevel)
while getopts 's:i:' opt; do
  case $opt in
  s) size=$OPTARG ;;
  i) images+=("$(realpath "$OPTARG")"); attach+=(--image "${images[-1]}") ;;
  *) echo "usage: paint.sh [-s SIZE] [-i IMAGE]... OUT.png PROMPT.txt" >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
[ $# -eq 2 ] || { echo "usage: paint.sh [-s SIZE] [-i IMAGE]... OUT.png PROMPT.txt" >&2; exit 2; }
target=$1
record=$(realpath "$2")
subject=$(jq -Rrs '
  def input: . == "none" or . == "unknown"
    or test("^unknown \"(?:[^\"\\\\]|\\\\.)*\"$")
    or test("^sha256 [0-9a-fA-F]{64} \"(?:[^\"\\\\]|\\\\.)*\"$");
  split("\n\nImage inputs:\n")
  | if length > 1 and (.[-1] | rtrimstr("\n") | split("\n") | all(input))
    then .[:-1] else . end
  | join("\n\nImage inputs:\n") | rtrimstr("\n")
' "$record")
paths=$(printf '%s\0' "${images[@]}" | jq -Rs 'split("\u0000") | map(select(length > 0))')
prompt="Generate exactly one square image with the image generation tool, every attached image as its reference, and the text between the markers below as the tool's prompt, passed unchanged, character for character, with nothing added before or after it. Use exactly this expression with literal JSON arguments: generatedImage(await tools.image_gen__imagegen({\"prompt\": <the exact text>, \"referenced_image_paths\": $paths})); An execution header may precede the expression; omit num_last_images_to_include and all other code. Then stop: do not judge, retry, edit, or describe the image, and write no files.
<<<PROMPT
$subject
PROMPT>>>"

inputs() {
  local encoded path relative hash
  while IFS= read -r encoded; do
    path=$(printf '%s' "$encoded" | base64 -d)
    path=$(realpath -e "$path") || return 1
    relative=$(realpath --relative-to="$root" "$path")
    case $relative in ../* | ..) echo "image input outside repository: $path" >&2; return 1 ;; esac
    hash=$(sha256sum < "$path") || return 1
    printf 'sha256 %s %s\n' "${hash%% *}" "$(jq -Rn --arg p "$relative" '$p')"
  done < <(jq -r '.[] | @base64' <<<"$1")
}
expected=$(inputs "$paths")

received() {
  local thread=$1 rollout
  local roots=("$codex_root/sessions")
  [ ! -d "$HOME/.codex-accounts" ] || roots+=("$HOME/.codex-accounts")
  rollout=$(find "${roots[@]}" -name "rollout-*-$thread.jsonl" -print -quit 2>/dev/null)
  [ -s "$rollout" ] || { echo "thread $thread has no rollout" >&2; return 1; }
  jq -sce '
    [ .[] | select(.type == "response_item") | .payload
      | select(.type == "custom_tool_call" and (.input | type) == "string")
      | .input | select(test("image_gen__imagegen"))
    ] | if length == 1 then .[0] else error("expected one image tool call") end
    | capture("^\\s*(?:// @exec:[^\\n]*\\n)?\\s*generatedImage\\(\\s*await\\s+tools\\.image_gen__imagegen\\s*\\(\\s*(?<args>\\{.*\\})\\s*\\)\\s*\\)\\s*;?\\s*$"; "ms").args | fromjson
    | if (.prompt | type) == "string"
        and (.referenced_image_paths // [] | type) == "array"
        and all(.referenced_image_paths[]?; type == "string")
        and (.num_last_images_to_include // 0) == 0
      then . else error("image inputs must be explicit paths") end
  ' "$rollout"
}

one() {
  local events thread srcs w h got actual part
  events=$(codex exec --skip-git-repo-check --json "$prompt" "${attach[@]}" </dev/null)
  thread=$(printf '%s\n' "$events" | jq -r 'select(.type == "thread.started") | .thread_id' | head -1)
  [ -n "$thread" ] || { printf '%s\n' "$events" >&2; return 1; }
  srcs=("$codex_root/generated_images/$thread"/*.png)
  [ "${#srcs[@]}" -eq 1 ] || { echo "thread $thread holds ${#srcs[@]} images, not one" >&2; return 1; }
  got=$(received "$thread") || return 1
  jq -e --arg p "$subject" '.prompt == $p' <<<"$got" >/dev/null || { printf 'thread %s: the image tool received another prompt:\n%s\n' "$thread" "$(jq -r .prompt <<<"$got")" >&2; return 1; }
  actual=$(inputs "$(jq -c '.referenced_image_paths // []' <<<"$got")") || return 1
  [ "$actual" = "$expected" ] || { echo "thread $thread: the image tool received other image inputs" >&2; return 1; }
  read -r w h < <(identify -format '%w %h\n' "${srcs[0]}")
  [ -n "$h" ] && [ "$w" = "$h" ] || { echo "thread $thread painted ${w}x${h}, not a square" >&2; return 1; }
  magick "${srcs[0]}" -resize "${size}x${size}" -strip png:- | pngquant --quality 70-95 --speed 1 - > "$target.part" || { echo "thread $thread: resize or quantise failed" >&2; return 1; }
  part="$record.$thread.part"
  jq -jr --arg inputs "${actual:-none}" '.prompt + "\n\nImage inputs:\n" + $inputs + "\n"' <<<"$got" > "$part" || return 1
  mv "$part" "$record" || return 1
  mv "$target.part" "$target"
}

mkdir -p "$(dirname "$target")"
rm -f "$target"
attempts=4
for attempt in $(seq 1 "$attempts"); do
  rm -f "$target.part"
  one && break
  if [ "$attempt" -eq "$attempts" ]; then
    echo "$target: gave up after $attempts attempts" >&2
    exit 1
  fi
  wait=$((1 << (attempt - 1)))
  echo "$target: attempt $attempt of $attempts failed, retrying in ${wait}s" >&2
  sleep "$wait"
done
[ -s "$target" ] || { echo "$target: empty" >&2; exit 1; }
