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
- Inside the silhouette, layer 2 alone matches the reference within a mean |ΔRGB| of 6.9 / 7.1 / 7.1; the four stacked miss it by 26.7 / 37.8 / 31.8, because layer 1 covers layer 2.
- The object split of the same picture returns its object layer lighter than the picture (25.9 / 23.0 / 32.9 inside the silhouette), which is why the matte takes colour from the Design picture and only alpha from the layer.

Verdict: Design-Layer splits by object only; it does not separate light terms. Evidence for a later decision, not a stage here.

## The set

[art/machines/sheet.png](art/machines/sheet.png) is every kept sprite on clay with its attempt and verdict. [proofs/ming-set-rig-162.png](proofs/ming-set-rig-162.png) is the `rig` shot scene with the new art loaded: the source, bonder and second bond with atoms in their seats, and every machine in the inventory column.

Attempts were painted ahead of measurement, so machines whose early attempt would have ended the run still painted four; `attempts.tsv` records the measured ones. Model time is the sum of both calls' wall times for every attempt painted; the listed price and the account usage read $0 before the first call and after the last.

| machine | attempts painted | kept | verdict of the kept attempt | model time (s) | cost |
|---|---|---|---|---|---|
| arm | 4 | 4 | pass | 408 | $0 |
| arm-2 | 4 | 2 | pass | 304 | $0 |
| arm-3 | 4 | 2 | fail outside 0.152 > 0.05 | 331 | $0 |
| bonder | 4 | 1 | fail outside 0.111 > 0.05 | 421 | $0 |
| converter-amber | 4 | 4 | fail outside 0.431 > 0.05 | 406 | $0 |
| converter-cobalt | 4 | 1 | fail outside 0.155 > 0.05 | 405 | $0 |
| output-1 | 4 | 1 | pass | 383 | $0 |
| output-2 | 4 | 1 | pass | 409 | $0 |
| output-3 | 4 | 1 | pass | 334 | $0 |
| portal | 3 | 1 | pass | 392 | $0 |
| reification | 4 | 4 | fail outside 0.257 > 0.05 | 440 | $0 |
| resonator | 4 | 4 | fail outside 0.174 > 0.05 | 422 | $0 |
| second-bond | 4 | 2 | fail outside 0.094 > 0.05 | 382 | $0 |
| source | 4 | 4 | fail outside 0.133 > 0.05 | 392 | $0 |
| source-2 | 4 | 3 | fail off_centre 0.152 > 0.1 | 434 | $0 |
| all 15 | 59 | | 6 pass, 9 kept closest | 5864 | $0 |

Where the seats and the body disagree, seat registration wins and the body overflows its footprint by the recorded `outside`: the model draws apertures closer together than the cells, so matching the seats scales the body past the envelope. Outputs and the portal, registered by silhouette alone, pass.
