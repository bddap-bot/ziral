#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
work=$(mktemp -d "$root/.paint-test.XXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/codex/sessions" "$work/codex/generated_images/recorded"
magick -size 64x64 xc:'#00ff00' "$work/input one.png"
magick -size 64x64 xc:'#ff0000' "$work/second.png"
cp "$work/input one.png" "$work/codex/generated_images/recorded/result.png"
cat > "$work/bin/codex" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
n=$(cat "$CODEX_HOME/attempts" 2>/dev/null || echo 0)
n=$((n + 1))
printf '%s' "$n" > "$CODEX_HOME/attempts"
[ "$n" -ge "${PAINT_TEST_PASS_ON:-1}" ] || { echo "codex: boom $n" >&2; exit 1; }
cp "$PAINT_TEST_LOG" "$CODEX_HOME/sessions/rollout-test-recorded.jsonl"
if [ "$PAINT_TEST_CHANGE" = hash ]; then
  printf changed >> "$PAINT_TEST_IMAGE"
fi
printf '%s\n' '{"type":"thread.started","thread_id":"recorded"}'
STUB
printf '#!/bin/sh\nexit 0\n' > "$work/bin/sleep"
chmod +x "$work/bin/"*
export PATH="$work/bin:$PATH" CODEX_HOME="$work/codex"
export PAINT_TEST_LOG="$work/event.jsonl" PAINT_TEST_IMAGE="$work/input one.png" PAINT_TEST_CHANGE=''
caption=$'A caption with "quotes", {braces}, and a second line.\nThe same image.\n\nImage inputs:\nThese words belong to the caption.'
jq --arg first "$work/input one.png" --arg second "$work/second.png" '.payload.input |= (gsub("INPUT"; $first) | gsub("SECOND"; $second))' art/tests/paint.jsonl > "$work/original.jsonl"
cp "$work/original.jsonl" "$PAINT_TEST_LOG"
printf '%s\n' "$caption" > "$work/prompt.txt"
art/paint.sh -s 64 -i "$work/input one.png" -i "$work/second.png" "$work/out.png" "$work/prompt.txt" > "$work/stdout" 2> "$work/stderr"
{
  printf '%s\n\nImage inputs:\n' "$caption"
  for path in "$work/input one.png" "$work/second.png"; do
    hash=$(sha256sum < "$path")
    printf 'sha256 %s %s\n' "${hash%% *}" "$(jq -Rn --arg path "${path#"$root/"}" '$path')"
  done
} > "$work/expected.txt"
cmp "$work/expected.txt" "$work/prompt.txt"
art/paint.sh -s 64 -i "$work/input one.png" -i "$work/second.png" "$work/out.png" "$work/prompt.txt" > "$work/stdout" 2> "$work/stderr"
cmp "$work/expected.txt" "$work/prompt.txt"
printf '%s\n' 'PASS capture_writes_recorded_prompt_and_image_paths_with_hashes'
for change in missing extra order hash; do
  PAINT_TEST_CHANGE=$change
  export PAINT_TEST_CHANGE
  jq --arg change "$change" '.payload.input |= (
    capture("image_gen__imagegen\\((?<args>.*)\\)\\);"; "m").args | fromjson
    | .referenced_image_paths |= (
        if $change == "missing" then .[:1]
        elif $change == "extra" then . + .[:1]
        elif $change == "order" then reverse
        else . end)
    | "generatedImage(await tools.image_gen__imagegen(" + tojson + "));")' "$work/original.jsonl" > "$PAINT_TEST_LOG"
  if art/paint.sh -s 64 -i "$work/input one.png" -i "$work/second.png" "$work/out.png" "$work/prompt.txt" > "$work/stdout" 2> "$work/stderr"; then
    echo "FAIL attachment_mismatch_rejects_capture: $change" >&2
    exit 1
  fi
  grep -q 'the image tool received other image inputs' "$work/stderr"
  [ ! -e "$work/out.png" ]
  cmp "$work/expected.txt" "$work/prompt.txt"
done
printf '%s\n' 'PASS attachment_mismatch_rejects_capture (missing, extra, order, content hash)'

PAINT_TEST_CHANGE=''
export PAINT_TEST_CHANGE
for change in prompt wrapper multiple recent; do
  jq --arg change "$change" '.payload.input |= (
    if $change == "wrapper" then "// " + . + "\nconst args = {}; await tools.image_gen__imagegen(args);"
    elif $change == "multiple" then . + "\n" + .
    else capture("image_gen__imagegen\\((?<args>.*)\\)\\);"; "m").args | fromjson
      | (if $change == "prompt" then .prompt = "Another caption" else .num_last_images_to_include = 1 end)
      | "generatedImage(await tools.image_gen__imagegen(" + tojson + "));" end)' "$work/original.jsonl" > "$PAINT_TEST_LOG"
  if art/paint.sh -s 64 -i "$work/input one.png" -i "$work/second.png" "$work/out.png" "$work/prompt.txt" > "$work/stdout" 2> "$work/stderr"; then
    echo "FAIL ambiguous_or_changed_call_rejects_capture: $change" >&2
    exit 1
  fi
  [ ! -e "$work/out.png" ]
  cmp "$work/expected.txt" "$work/prompt.txt"
done
printf '%s\n' 'PASS ambiguous_or_changed_call_rejects_capture (prompt, wrapper, multiple, recent images)'

cp "$work/codex/generated_images/recorded/result.png" "$work/input one.png"
cp "$work/original.jsonl" "$PAINT_TEST_LOG"
rm "$work/bin/sleep"
for pass_on in 3 99; do
  export PAINT_TEST_PASS_ON=$pass_on
  rm "$CODEX_HOME/attempts"
  started=$SECONDS
  if art/paint.sh -s 64 -i "$work/input one.png" -i "$work/second.png" "$work/out.png" "$work/prompt.txt" > "$work/stdout" 2> "$work/stderr"; then
    [ "$pass_on" -eq 3 ]
    [ "$(cat "$CODEX_HOME/attempts")" -eq 3 ]
    [ "$((SECONDS - started))" -ge 3 ]
    grep -q 'codex: boom 1' "$work/stderr"
    grep -q 'attempt 2 of 4 failed, retrying in 2s' "$work/stderr"
  else
    [ "$pass_on" -eq 99 ]
    [ "$(cat "$CODEX_HOME/attempts")" -eq 4 ]
    [ "$((SECONDS - started))" -ge 7 ]
    grep -q 'attempt 3 of 4 failed, retrying in 4s' "$work/stderr"
    grep -q 'gave up after 4 attempts' "$work/stderr"
    [ ! -e "$work/out.png" ]
  fi
done
printf '%s\n' 'PASS paint_retries_with_backoff_and_reports_failed_attempts'
unset PAINT_TEST_PASS_ON

cp art/textures/atom-base.prompt.txt "$work/shipped.txt"
jq -Rs 'split("\n\nImage inputs:\n")[0] | rtrimstr("\n")' art/textures/atom-base.prompt.txt > "$work/caption.json"
jq --slurpfile caption "$work/caption.json" '.payload.input |= (
  capture("image_gen__imagegen\\((?<args>.*)\\)\\);"; "m").args | fromjson
  | .prompt = $caption[0]
  | "generatedImage(await tools.image_gen__imagegen(" + tojson + "));")' "$work/original.jsonl" > "$PAINT_TEST_LOG"
art/textures/gen.sh -o "$work/output" -s 64 -i "$work/input one.png" -i "$work/second.png" atom-base > "$work/stdout" 2> "$work/stderr"
cmp "$work/shipped.txt" art/textures/atom-base.prompt.txt
[ -s "$work/output/atom-base.png" ]
grep -q '^sha256 ' "$work/output/atom-base.prompt.txt"
art/textures/gen.sh -o "$work/output" -n 2 -s 64 -i "$work/input one.png" -i "$work/second.png" atom-base > "$work/stdout" 2> "$work/stderr"
for i in 00 01; do
  [ -s "$work/output/atom-base-$i.png" ]
  cmp "$work/output/atom-base.prompt.txt" "$work/output/atom-base-$i.prompt.txt"
done
cmp "$work/shipped.txt" art/textures/atom-base.prompt.txt
cp "$work/output/atom-base.prompt.txt" "$work/alternate.txt"
if art/textures/gen.sh -o "$work/output" -s 64 -i "$work/missing.png" atom-base > "$work/stdout" 2> "$work/stderr"; then
  echo "FAIL alternate_texture_output_keeps_its_own_prompt: missing input" >&2
  exit 1
fi
cmp "$work/alternate.txt" "$work/output/atom-base.prompt.txt"
[ -s "$work/output/atom-base.png" ]
printf '%s\n' 'PASS alternate_texture_output_keeps_its_own_prompt'
