#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
scratch=${SCRATCH:-$HOME/scratch/normal-probe-run}
mkdir -p "$scratch"
py=$(nix-build --no-out-link '<nixpkgs>' -A python3)/bin/python3
libs=$(nix-build --no-out-link '<nixpkgs>' -A stdenv.cc.cc.lib -A zlib | sed 's|$|/lib|' | paste -sd:)
cp "$here/marigold.py" "$scratch/"
cat > "$scratch/inner.sh" <<INNER
set -e
cd "$scratch"
export HF_HOME=\$PWD/hf PIP_CACHE_DIR=\$PWD/pipcache PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True
[ -x venv/bin/python ] || {
  $py -m venv venv
  venv/bin/pip install -q --index-url https://download.pytorch.org/whl/cu124 torch==2.6.0 torchvision
  venv/bin/pip install -q diffusers==0.40.0 transformers accelerate pillow numpy
}
export CPU=${CPU:-} OUT=${OUT:-outA}
mkdir -p "\$OUT"
venv/bin/python marigold.py "\$@"
INNER
cd "$scratch"
LD_LIBRARY_PATH="$libs:/run/opengl-driver/lib" run-untrusted -g bash inner.sh "$@"
