#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
scratch=$(mktemp -d "$here/.gemini-test.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/bin" "$scratch/run"
magick -size 32x32 xc:none -fill '#886633' -draw 'rectangle 4,4 27,27' -fill none -draw 'circle 16,16 16,12' "$scratch/run/layout.png"
magick -size 32x32 xc:white -fill '#886633' -draw 'rectangle 4,4 27,27' -fill '#ff00ff' -draw 'rectangle 13,13 19,19' "$scratch/returned.png"
base64 -w0 "$scratch/returned.png" > "$scratch/image.b64"
jq -n --rawfile image "$scratch/image.b64" '{usage:{cost:0.1},data:[{b64_json:$image}]}' > "$scratch/response.json"
cat > "$scratch/bin/curl" <<'CURL'
#!/usr/bin/env bash
set -euo pipefail
[ "${MOCK_FAIL:-0}" = 0 ] || exit 22
while [ "$#" -gt 0 ]; do
  if [ "$1" = -o ]; then cp "$MOCK_RESPONSE" "$2"; exit 0; fi
  shift
done
exit 1
CURL
chmod +x "$scratch/bin/curl"
export PATH="$scratch/bin:$PATH" OPENROUTER_API_KEY=offline MOCK_RESPONSE="$scratch/response.json"
printf 'A brass machine.\n' > "$scratch/caption.txt"
"$here/gemini-key.sh" "$scratch/caption.txt" "$scratch/run/image.png"
[ "$(magick "$scratch/run/image.png" -format '%[fx:p{0,0}.a] %[fx:p{16,16}.a] %[fx:p{8,8}.a]' info:)" = '0 0 1' ]
[ "$(jq '.usage.cost' "$scratch/run/response-metadata.json")" = 0.1 ]
set +e
MOCK_FAIL=1 "$here/gemini-key.sh" "$scratch/caption.txt" "$scratch/run/failure.png"
status=$?
set -e
[ "$status" -eq 3 ]
[ ! -e "$scratch/run/failure.png" ]
echo 'PASS: key transparency, exterior transparency, body preservation, actual cost, and no paid retry on request failure'
