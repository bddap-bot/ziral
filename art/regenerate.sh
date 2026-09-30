#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
started=$SECONDS
cargo build --release
printf 'initial-build\t%ss\n' "$((SECONDS - started))"
binary="${CARGO_TARGET_DIR:-target}/release/ziral"
stage=$SECONDS
"$binary" --gen --all "$@"
printf 'generation-and-packing\t%ss\n' "$((SECONDS - stage))"
stage=$SECONDS
cargo build --release
printf 'embed-updated-art\t%ss\n' "$((SECONDS - stage))"
mkdir -p proofs
stage=$SECONDS
"$binary" --shot proofs/machine-set-rig.png rig 8 > proofs/regenerate-render.log 2>&1 || {
  cat proofs/regenerate-render.log >&2
  exit 1
}
cat proofs/regenerate-render.log
if grep -iEq 'ERROR.*(shader|pipeline|wgpu|naga|bevy_render)' proofs/regenerate-render.log; then exit 1; fi
[ -s proofs/machine-set-rig.png ]
printf 'render\t%ss\n' "$((SECONDS - stage))"
printf 'regenerate: full set, sheet and rig screenshot complete in %ss\n' "$((SECONDS - started))"
