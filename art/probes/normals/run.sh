#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
scratch=${SCRATCH:-$HOME/scratch/normal-probe-run}
tex=$here/../../textures
tiles=(arm bonder second-bond source output tile-00 tile-01)
masters=(tile-00 second-bond arm)
directions=(right "top right" left bottom)
cd "$scratch"
mkdir -p in A B out

for t in "${tiles[@]}"; do cp "$tex/$t.png" in/; done
"$here/estimate.sh" 512 4 1 in/*.png
for t in "${tiles[@]}"; do "$here/probe.py" albedo "in/$t.png" "outA/$t.npy" "A/$t"; done

edit() {
  local master=$1 dir=$2 out="B/$1-${2/ /-}"
  [ -f "$out.png" ] && return
  local p="Use the image generation tool once to edit the attached image, then stop: do not judge, retry, describe, or write files. Edit: the same image with identical geometry, materials, colours, texture detail, and the grey sphere in the lower-right corner kept exactly where it is; only the lighting changes: it is now lit by one warm raking light coming from the $dir, so every ridge, bump, and the sphere are shaded and highlighted accordingly."
  local events thread
  events=$(printf '%s' "$p" | codex exec --skip-git-repo-check --json -i "$PWD/B/$master-master.png" -)
  printf '%s\n' "$events" > "$out.jsonl"
  thread=$(printf '%s\n' "$events" | jq -r 'select(.type == "thread.started") | .thread_id' | head -1)
  cp "$HOME/.codex/generated_images/$thread/"*.png "$out.png"
}
for m in "${masters[@]}"; do
  [ -f "B/$m-master.png" ] || "$here/probe.py" sphere "in/$m.png" "B/$m-master.png"
  for d in "${directions[@]}"; do edit "$m" "$d"; done
  set=("B/$m-master.png")
  for d in "${directions[@]}"; do
    f="B/$m-${d/ /-}"
    magick "$f.png" -resize 1024x1024! "$f-1024.png"
    set+=("$f-1024.png")
  done
  "$here/probe.py" lights "${set[@]}"
  "$here/probe.py" ps "B/$m" "${set[@]}"
done

for t in "${tiles[@]}"; do
  "$here/probe.py" render "out/control-$t" "in/$t.png" - 200
  "$here/probe.py" render "out/A-$t" "A/$t-albedo.png" "A/$t.npy" 200
done
for m in "${masters[@]}"; do "$here/probe.py" render "out/B-$m" "B/$m-albedo.png" "B/$m.npy" 200; done
sheet() {
  local name=$1; shift
  magick "$@" -background '#6B4F3A' -append png:- | pngquant --quality 70-95 --speed 1 - > "$here/$name.png"
}
sheet control out/control-{arm,bonder,second-bond,source,output,tile-00,tile-01}.png
sheet route-a out/A-{arm,bonder,second-bond,source,output,tile-00,tile-01}.png
sheet route-b out/B-{tile-00,second-bond,arm}.png
for m in "${masters[@]}"; do
  magick "B/$m-master.png" "B/$m-right-1024.png" "B/$m-top-right-1024.png" "B/$m-left-1024.png" "B/$m-bottom-1024.png" "B/$m-normal.png" "B/$m-albedo.png" -resize 200x200 +append png:- | pngquant --quality 70-95 --speed 1 - > "$here/route-b-$m-inputs.png"
done
magick A/{arm,bonder,second-bond,source,output,tile-00,tile-01}-normal.png -resize 200x200 +append png:- | pngquant --quality 70-95 --speed 1 - > "$here/route-a-normals.png"
