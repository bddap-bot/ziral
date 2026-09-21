#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
work=$(mktemp -d "$root/.ask-test.XXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/codex/sessions"
cat > "$work/bin/codex" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$@" > "$CODEX_HOME/args"
model=''
while [ $# -gt 0 ]; do
  case $1 in
    --model) model=$2; shift ;;
    --ephemeral) exit 2 ;;
  esac
  shift
done
[ "$model" = "$(cat art/director-model.txt)" ] || exit 2
log="$CODEX_HOME/sessions/rollout-test-recorded.jsonl"
rm -f "$log"
case $ASK_TEST_MODE in
  missing) ;;
  empty) printf '%s\n' '{"type":"session_meta"}' > "$log" ;;
  *)
    jq -nc --arg model "$model" '{type:"turn_context",payload:{model:$model}}' > "$log"
    case $ASK_TEST_MODE in
      wrong) printf '%s\n' '{"type":"turn_context","payload":{"model":"other"}}' > "$log" ;;
      mixed) printf '%s\n' '{"type":"turn_context","payload":{"model":"other"}}' >> "$log" ;;
    esac
    ;;
esac
printf '%s\n' '{"type":"thread.started","thread_id":"recorded"}' '{"type":"item.completed","item":{"type":"agent_message","text":"accepted"}}'
[ "$ASK_TEST_MODE" != failed ]
STUB
printf '#!/bin/sh\nexit 0\n' > "$work/bin/sleep"
chmod +x "$work/bin/"*
export PATH="$work/bin:$PATH" CODEX_HOME="$work/codex" HOME="$work"
export ASK_TEST_MODE=matching
art/ask.sh '{}' 'Write a caption.' > "$work/stdout" 2> "$work/stderr"
[ "$(cat "$work/stdout")" = accepted ]
printf '%s\n' 'PASS explicit_director_model_and_matching_transcript'
for ASK_TEST_MODE in wrong mixed empty missing failed; do
  export ASK_TEST_MODE
  if art/ask.sh '{}' 'Write a caption.' > "$work/stdout" 2> "$work/stderr"; then
    echo "FAIL accepted $ASK_TEST_MODE transcript" >&2
    exit 1
  fi
  [ ! -s "$work/stdout" ]
  grep -q 'gave up after 4 attempts' "$work/stderr"
  case $ASK_TEST_MODE in
    wrong | mixed | empty) grep -q 'transcript model must be' "$work/stderr" ;;
    missing) grep -q 'has no rollout' "$work/stderr" ;;
  esac
  printf 'PASS rejects_%s\n' "$ASK_TEST_MODE"
done

mv "$work/codex/sessions" "$work/codex/actual-sessions"
ln -s actual-sessions "$work/codex/sessions"
export ASK_TEST_MODE=matching
art/ask.sh '{}' 'Write a caption.' > "$work/stdout" 2> "$work/stderr"
[ "$(cat "$work/stdout")" = accepted ]
printf '%s\n' 'PASS follows_sessions_directory_symlink'
