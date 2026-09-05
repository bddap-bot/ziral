# Normal-map probe for rotatable machine tiles

Part of #19. Question: how to obtain a usable normal map for a diffusion-generated
tile so one world light can be applied at runtime and rotation tweens are exact.

Inputs: `art/textures/{arm,bonder,second-bond,source,output,tile-00,tile-01}.png`
(1024², generated with a soft light from the top-left). Every render here uses one fixed
world light `(-0.5, 0.5, 0.7)`, ambient 0.25, shading normalised so a flat facing
surface renders as its albedo. Rotations rotate the normal vectors along with the
pixels (`probe.py render`), not the pixels alone.

## Route A: monocular normal estimation (Marigold normals)

`prs-eth/marigold-normals-v1-1` through `diffusers` 0.40.0 (`MarigoldNormalsPipeline`),
`torch` 2.6.0+cu124, fp16. Chosen because it is pip-installable with no repo checkout
and its weights are on the Hub with no gate; StableNormal and DSINE need their own
repos and Google-Drive weights. Runs inside `run-untrusted -g` (bubblewrap with the
GPU passed through; `HF_HOME` and the venv live in the scratch dir). The RTX 2080
shares the box with a game holding ~3.8 GB, so inference is at processing resolution
512, 4 steps, ensemble 1 (`estimate.sh 512 4 1`), peak 3.1 GB. Then
`probe.py albedo`: the mean normal is rotated to +z (the swatch is a flat surface),
albedo = image ÷ shade.

Timings: model load 3–4 s, 0.6–1.1 s per 1024² tile on the GPU. CPU fp32 at
processing resolution 1024, 10 steps: 575 s per tile (arm), not used for the sheet.

Result: flat. The estimator sees a macro swatch as one plane and returns a
near-constant normal (per-channel std 0.005–0.026 on six of seven tiles; tile-00,
which has a real ridge, gets 0.07). Correlation between the estimated shade and the
tile's own luminance is ~0 (−0.37 … +0.10), i.e. the map does not explain the baked
lighting, so the "albedo" still carries it and the render is indistinguishable from
the control. The estimator is non-deterministic run to run; the sheet is from
`run.log.txt`'s run.

## Route B: photometric stereo via relighting edits

Master = tile with a matte grey Lambertian sphere (radius 96 px, albedo 0.6)
composited into the lower-right corner (`probe.py sphere`). Four edit passes per
master through the Codex built-in image tool exactly as `art/textures/gen.sh`
(`codex exec --skip-git-repo-check --json -i <master> -`, thread id →
`~/.codex/generated_images/<thread>/*.png`; the model returns 1254², resized to
1024²), asking for identical geometry and materials relit from the right, top right,
left, bottom. ~35–48 s per edit, four edits per tile. Masters: tile-00, second-bond,
arm. Per edit `probe.py lights` fits `lum = ρ (n·l) + ambient` on the sphere
(shadow pixels iteratively excluded), giving the actual light direction and the mean
absolute drift of the pixels outside the sphere against the master. `probe.py ps`
solves per-pixel least squares `L · (ρ n) = I` over master + four edits with the
recovered lights, flattens the mean normal to +z, and rescales ρ so the master
re-renders at its own brightness.

Recovered lights vs requested (`run.log.txt`): 33–60° off. The model keeps the light
in the upper hemisphere: "right" comes back as upper-right, and "bottom" comes back
as a grazing light from below the surface plane (z = −0.13 … +0.06, sphere fit rms
0.04–0.05, the worst of the set); it is clamped to z = 0.05 rather than dropped
because it is the only sample with negative y and the solve is otherwise
ill-conditioned. Light-matrix condition number 2.7–3.3 with it. Geometric drift:
mean abs pixel error 0.06–0.16 (on 0–1) outside the sphere; visible as the ridge
crumbs in tile-00 being repainted between edits, and as broad brightness gradients on
arm that the solve turns into low-frequency normal tilt.

Result: a viewer-acceptable map for tile-00 and second-bond (the ridge and crackle
keep their lit side on the world's light side through all six orientations); arm is
soft, with mild brightness wobble across orientations from the gradients above. The
sphere doubles as a built-in check: it comes back as a sphere in the normal map.

## Route C: six baked variants (costed, not run)

Under one world light, six light directions ≡ six orientations: six textures, not
thirty-six. A tween between two variants is a crossfade. The cost is six independent
generations disagreeing on detail so swaps pop; making them agree means edit passes
from one master, which is Route B's consistency problem with none of its payoff (no
normals, so no exact tween).

## Files

- `control.png`: original baked-lighting textures at the six orientations (rows =
  tiles, columns = 0°…300°). The highlight rotates with the tile.
- `route-a.png`: Route A albedo + normals under the world light, same layout.
- `route-a-normals.png`: Route A normal maps.
- `route-b.png`: Route B, rows tile-00, second-bond, arm.
- `route-b-<tile>-inputs.png`: master, the four edits, recovered normal, recovered
  albedo.
- `run.log.txt`: the numbers above, from the run that produced the sheets.

## Commands

```
nix-shell -p imagemagick pngquant python3 python3Packages.numpy python3Packages.pillow --run ./run.sh
```

`run.sh` copies the inputs to `$SCRATCH` (default `~/scratch/normal-probe-run`), runs
`estimate.sh` (creates the sandboxed venv on first use), the Codex edits (skipped when
the edit already exists), the solves, the renders, and the sheets. `probe.py` is the
one implementation of the compositor, light solver, photometric-stereo solve, and
renderer; `marigold.py` is the sandboxed estimator runner.
