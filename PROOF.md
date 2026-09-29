# Ming-Image art workflow (#162)

Every machine sprite on this branch comes from one chain: `caption.txt` → Ming-Image Design → Ming-Image Design-Layer → matte → fit and register → measure → `albedo.png` → relief and rig maps. [art/MACHINES.md](art/MACHINES.md) describes the stages.

## The asset contract

What the game loads per machine, read from `src/look.rs`, `src/rig.rs` and `src/main.rs` before generating:

| file | contract |
|---|---|
| `art/machines/<name>/albedo.png` | RGBA, square, side = the machine's quad at 256 px per hex (portal 512, arm/bonder/resonator 955, second-bond/amber converter 1024, cobalt converter 1280, arm-2/output-1 1399, source/source-2 1481, arm-3 1842, reification/output-2 2286, output-3 3172); alpha is the silhouette; seats sit on their cell centres |
| `art/machines/<name>/normal.png` | same size, tangent-space normal in RGB, alpha = the albedo's |
| `art/machines/<name>/parts/{albedo,normal,emissive}-{base,moving}.png` | only for machines whose manifest lists parts; same size; emissive alpha = albedo alpha |
| `art/machines/manifest.toml` | parsed at build time by `src/rig.rs` and `src/sound.rs` for parts, motion, emitters and instruments |

Names come from `look::machine`; every PNG is compiled in with `include_bytes!`, so a missing file fails the build.

## Deleted

- `art/direct.sh`, `art/ask.sh`, `art/director-model.txt`, `art/tests/ask.sh`: the director and critic model calls; a caption file per machine and measured retries replace them.
- The critic loop in `src/machines.rs`: rounds, rebriefs, judged boards, scores rows with critic columns, the rubric and threshold.
- Chroma-key removal (`KEY`, spill and unspill): Design-Layer returns real alpha.
- Per machine: `prompt.txt`, `candidates/`, `scores.tsv`, `scaffold.png` (the painter no longer takes a reference), `second-bond/recipe.png`.
- Manifest: `candidates`, `style.shared`, `style.arm`, `style.critic`, `thresholds.critic`, every `direction`, `references` and `briefed`.

## Kept

- `art/machines/manifest.toml`: the one source of rig parts, motion, particles, instruments, thresholds and cache keys.
- Registration and measurement in `src/machines.rs`: now the retry policy for the model's shape liberties.
- Relief and calibration relights: the runtime normal map comes from them, and atoms and bonds use the same path.
- `art/machines/rig.sh`: the game loads split part maps; the layer split separates by object, not by moving part.
- The sheet builder (`--gen` writes `art/machines/sheet.png`): the one view of the whole set.
- The game-side loaders (`src/look.rs`, `src/rig.rs`): unchanged; the chain writes their contract.
- `art/paint.sh` and `art/textures/gen.sh`: the tiles and ethereal surface are painted over the grout template and the manual page is an edit of a supplied page, reference images Design cannot take. Symbols and pips are vector sources rendered by script at exact slot geometry; a raster model does not beat them. Atoms and bonds could move to this chain; they stay on `paint.sh` in this pass.

## Lighting-split probe

One reference, the resonator's first Design picture, the same picture its object split used. The Design-Layer plan asked for four layers: flat albedo with no shading or highlights, specular highlights only, directional light and shadow only, and the background (plan and outputs in [proofs/ming-light-split-162.png](proofs/ming-light-split-162.png), last tile the recomposite).

- Layer 1 ("flat albedo") is a smoothed re-render of the body without the brass crest, rivets or crazing, still shaded and glossy: luminance standard deviation 30.1 over its opaque pixels.
- Layer 2 ("specular only") is the complete detailed object, opaque over the same silhouette: opaque IoU with layer 1 is 0.962.
- Layer 3 ("light and shadow") is a flat pale wash, RGB (225, 225, 227) at alpha 10 to 79, with no shading structure.
- Stacking the four layers misses the reference by a mean |ΔRGB| of 20.4 / 25.4 / 24.1, against 4.3 for the object split.

Verdict: Design-Layer splits by object only; it does not separate light terms. Evidence for a later decision, not a stage here.
