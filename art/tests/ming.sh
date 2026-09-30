#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
scratch=$(mktemp -d "$here/ming-test.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/bin"
cat > "$scratch/bin/curl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
case ${*: -1} in
*/credits)
  echo credits >> "$TRACE"
  case $CASE in
  pre-spent) echo '{"data":{"total_usage":0.01}}' ;;
  pre-unknown) exit 22 ;;
  post-spent|post-unknown)
    if grep -q '^post$' "$TRACE"; then
      [ "$CASE" != post-unknown ] || exit 22
      echo '{"data":{"total_usage":0.01}}'
    else echo '{"data":{"total_usage":0}}'; fi ;;
  *) echo '{"data":{"total_usage":0}}' ;;
  esac
  exit ;;
*/endpoints)
  echo pricing >> "$TRACE"
  if [ "$CASE" = price ]; then
    echo '{"endpoints":[{"pricing":[{"cost_usd":0.1}]}]}'
  else echo '{"endpoints":[{"pricing":[{"cost_usd":0}]}]}'; fi
  exit ;;
esac
echo post >> "$TRACE"
out=
while [ $# -gt 0 ]; do
  if [ "$1" = -o ]; then out=$2; shift; fi
  shift
done
case $CASE in
post-spent|post-unknown|http)
  echo '{"error":"failed"}' > "$out"
  printf 502 ;;
invalid-false|invalid-null)
  value=${CASE#invalid-}
  printf '{"usage":{"cost":%s},"data":[{"b64_json":"eA=="}]}' "$value" > "$out"
  printf 200 ;;
invalid-cost)
  echo '{"usage":{"cost":"unknown"},"data":[{"b64_json":"eA=="}]}' > "$out"
  printf 200 ;;
cost)
  echo '{"usage":{"cost":0.1},"data":[{"b64_json":"eA=="}]}' > "$out"
  printf 200 ;;
bad-image)
  echo '{"usage":{"cost":0},"data":[{"b64_json":"!"}]}' > "$out"
  printf 200 ;;
*)
  echo '{"usage":{"cost":0},"data":[{"b64_json":"eA=="}]}' > "$out"
  printf 200 ;;
esac
MOCK
chmod +x "$scratch/bin/curl"
export PATH="$scratch/bin:$PATH" OPENROUTER_API_KEY=test-only
export TRACE="$scratch/trace" CASE
export ZIRAL_GENERATOR_STOP="$scratch/stop"
printf 'a glazed object\n' > "$scratch/caption.txt"
for CASE in pre-spent pre-unknown price post-spent post-unknown cost invalid-cost invalid-false invalid-null http bad-image success; do
  : > "$TRACE"
  rm -f "$ZIRAL_GENERATOR_STOP"
  rm -f "$scratch/out.png"
  rc=0
  "$here/../ming.sh" design "$scratch/out.png" "$scratch/caption.txt" > "$scratch/log" 2> "$scratch/err" || rc=$?
  case $CASE in
  pre-spent|pre-unknown|price)
    [ "$rc" -eq 3 ]
    if grep -q '^post$' "$TRACE"; then exit 1; fi ;;
  post-spent|post-unknown|cost|invalid-cost|invalid-false|invalid-null)
    [ "$rc" -eq 3 ]
    [ "$(grep -c '^post$' "$TRACE")" -eq 1 ]
    [ "$(tail -1 "$TRACE")" = credits ] ;;
  http|bad-image)
    [ "$rc" -ne 0 ]
    [ "$(grep -c '^post$' "$TRACE")" -eq 1 ]
    [ "$(tail -1 "$TRACE")" = credits ] ;;
  success)
    [ "$rc" -eq 0 ]
    [ "$(cat "$scratch/out.png")" = x ]
    [ "$(tail -1 "$TRACE")" = credits ] ;;
  esac
  if [ "$rc" -eq 3 ]; then [ -s "$ZIRAL_GENERATOR_STOP" ]; fi
  if [ "$CASE" != success ]; then [ ! -e "$scratch/out.png" ]; fi
  printf '%s: pass\n' "$CASE"
done

: > "$TRACE"
printf 'stopped\n' > "$ZIRAL_GENERATOR_STOP"
rc=0
"$here/../ming.sh" design "$scratch/blocked.png" "$scratch/caption.txt" > "$scratch/log" 2> "$scratch/err" || rc=$?
[ "$rc" -eq 3 ]
[ ! -s "$TRACE" ]
[ ! -e "$scratch/blocked.png" ]
echo 'shared-stop: pass'
