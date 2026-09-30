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
- The calibration relights (`relit/` for every machine, atom and bond; the sphere renderer and its solver; `style.facings`, `style.elevation`, `thresholds.sphere`, the `[texture.*]` entries): their lighting measurements evaluated an analytic sphere rather than the sprite, and nothing at runtime loads them.
- The palette measurement: it was reported and never decided.
- `proofs/rounds/`: renders of deleted critic rounds.
- Per machine: `prompt.txt`, `candidates/`, `scores.tsv`, `scaffold.png` (the painter no longer takes a reference), `second-bond/recipe.png`.
- Manifest: `candidates`, `style.shared`, `style.arm`, `style.critic`, `thresholds.critic`, every `direction`, `references` and `briefed`.

## Kept

- `art/machines/manifest.toml`: the one source of rig parts, motion, particles, instruments, thresholds and cache keys.
- Registration and measurement in `src/machines.rs`: now the retry policy for the model's shape liberties.
- Surface normals from the albedo (`normal.png`): the shader lights the sprite through them.
- `art/machines/rig.sh`: the game loads split part maps; the layer split separates by object, not by moving part.
- The sheet builder (`--gen` writes `art/machines/sheet.png`): the one view of the whole set.
- The game-side loaders (`src/look.rs`, `src/rig.rs`): unchanged; the chain writes their contract.
- `art/paint.sh` and `art/textures/gen.sh`: the tiles and ethereal surface are painted over the grout template and the manual page is an edit of a supplied page, reference images Design cannot take. Symbols and pips are vector sources rendered by script at exact slot geometry; a raster model does not beat them. Atoms and bonds are not regenerated in this pass and keep their `paint.sh` textures; moving them is the one swap left open.

## Lighting-split probe

One reference, the resonator's first Design picture, the same picture its object split used. The Design-Layer plan asked for four layers: flat albedo with no shading or highlights, specular highlights only, directional light and shadow only, and the background (reference, returned layers and recomposite in [proofs/ming-light-split-162.png](proofs/ming-light-split-162.png), last tile the recomposite).


```text
Decompose this image into 4 layers with the following specifications:

Number of layers: 4
Layer 1: Flat albedo: the object with its true surface colours only, evenly lit, with no shading, no highlights and no shadows.
Layer 2: Specular highlights only: the glossy reflections and bright glints on the glaze and brass, and nothing else.
Layer 3: Directional light and shadow only: the shading across the object from the overhead light, darker where surfaces turn away, and nothing else.
Layer 4: The plain background.
```

- Layer 1 ("flat albedo") is a smoothed re-render of the body without the brass crest, rivets or crazing, still shaded and glossy: luminance standard deviation 30.1 over its opaque pixels.
- Layer 2 ("specular only") is the complete detailed object, opaque over the same silhouette: opaque IoU with layer 1 is 0.962.
- Layer 3 ("light and shadow") is a flat pale wash, RGB (225, 225, 227) at alpha 10 to 79, with no shading structure.
- Inside the silhouette, layer 2 alone matches the reference within a mean |ΔRGB| of 6.9 / 7.1 / 7.1; the four stacked miss it by 26.7 / 37.8 / 31.8, because layer 1 covers layer 2.
- The object split of the same picture returns its object layer lighter than the picture (25.9 / 23.0 / 32.9 inside the silhouette), which is why the matte takes colour from the Design picture and only alpha from the layer.

Verdict: in this probe, Design-Layer splits by object only; it does not separate light terms. Evidence for a later decision, not a stage here.

## The set

[art/machines/sheet.png](art/machines/sheet.png) is every kept sprite on clay with its attempt and verdict. [proofs/ming-set-rig-162.png](proofs/ming-set-rig-162.png) is the `rig` shot scene with the new art loaded: the source, bonder and second bond with atoms in their seats, and every machine in the inventory column.

This run painted attempts ahead of measurement, so machines whose early attempt ended the run still painted more; the committed loop measures each attempt before painting the next, and `attempts.tsv` records the measured ones. Model time is the sum of both calls' wall times for every attempt painted; the recorded prices and costs were $0. Account total_usage was 0 before generation, after the run, and on recovery. The script checked credits around each successful call; the revised script checks immediately after failed calls too. The run made 59 two-stage attempts for 15 machine assets; its summed request wall time is listed below. The final fitting correction reprocessed every cached design without an API key and made no model calls. Cached processing times below are rounded per-machine wall times for registration, selection and normals; sheet assembly and rig splitting are additional.

| machine | attempts painted | kept | verdict of the kept attempt | model time (s) | cached processing (s) | cost |
|---|---|---|---|---|---|---|
| arm | 4 | 4 | pass | 408.3 | 3 | $0 |
| arm-2 | 4 | 2 | pass | 303.8 | 3 | $0 |
| arm-3 | 4 | 2 | fail outside 0.152 > 0.05 | 331.3 | 5 | $0 |
| bonder | 4 | 1 | fail outside 0.111 > 0.05 | 420.7 | 4 | $0 |
| converter-amber | 4 | 4 | fail outside 0.413 > 0.05 | 406.3 | 4 | $0 |
| converter-cobalt | 4 | 1 | fail outside 0.155 > 0.05 | 405.3 | 4 | $0 |
| output-1 | 4 | 1 | pass | 383.1 | 1 | $0 |
| output-2 | 4 | 1 | pass | 409.4 | 3 | $0 |
| output-3 | 4 | 1 | pass | 333.9 | 6 | $0 |
| portal | 3 | 1 | pass | 392.2 | 1 | $0 |
| reification | 4 | 3 | fail outside 0.198 > 0.05 | 440.3 | 14 | $0 |
| resonator | 4 | 4 | fail outside 0.174 > 0.05 | 421.5 | 4 | $0 |
| second-bond | 4 | 2 | fail outside 0.090 > 0.05 | 382.2 | 4 | $0 |
| source | 4 | 4 | fail outside 0.175 > 0.05 | 391.8 | 5 | $0 |
| source-2 | 4 | 3 | fail seat 0.073 < 0.1 | 433.9 | 6 | $0 |
| all 15 | 59 | | 6 pass, 9 kept closest | 5864.0 | 67 | $0 |

Every failing verdict is on a machine registered by its seats; every machine registered by silhouette alone (outputs, portal) passes. Seat registration sets the scale from the model's aperture spacing, and wherever that spacing is tighter than the cells relative to the body, the body lands past its footprint by the recorded `outside`.


## Review and scope

The retained tile painter accepts reference images for the grout and manual page. The Ming machine painter has one retry loop; removing its nested HTTP retries and serialising generation eliminates a second budget loop, admission races and concurrent full-resolution image buffers. `curl` carries HTTP, `jq` builds and validates JSON, ImageMagick assembles the sheet and rig masks, and pngquant writes the existing compact sprite format. All are used by a retained stage.

The first review found delayed credit checks, lost failed-stage timing, an admission race, and a swallowed sheet error. The corrections check credits immediately after every request, preserve stage logs, process one machine at a time, and propagate sheet failure. Offline transport fixtures cover nonzero or unavailable credits, changed price, charged responses, HTTP failures and malformed image data without making API calls.

The set is a branch experiment. Nine best candidates still miss the existing geometry thresholds; those verdicts remain on the sheet. Strong painted shading remains visible. The exact slot-aligned instruction vectors, inventory pips and reference-aligned manual overlay remain; no controlled model comparison was generated for these optional classes. Atoms, bonds and board tiles remain outside this machine-generation pass. No main-branch or deployment claim is made.

Review scores before corrections: correctness 7/10, premise/design 7/10, security/prose 6/10. The semantic-delta security/correctness review reached 8.5/10 and then 9/10 after spend classification survived log-write failure and invalid costs were refused. The final `false`/`null` cost cases now have explicit refusal fixtures. The mutation pass killed removal of the preflight credits check, post-call credits check and price guard (baseline exit 0, each mutant exit 1). It first exposed an ineffective negated-grep assertion, which was corrected. Rust fixtures also exercise real stage-log persistence and propagation of a sheet-write failure.

Recovery also found an interrupted rig split: both sources and three second-bond maps still described the preceding sprites. Regenerating all articulated maps repaired them. The shipped-asset test now checks part colours and combined alpha against each current whole albedo and normal map, so a partial rig update cannot silently pass.

The machine suite exposed a fitting defect on asymmetric footprints: the average cell position differs from the centre of the footprint bounds. Fitting now aligns bounding centres before seat registration. The existing clean/translated-seat test caught the defect; it remains in the suite. Measurement-only tests score unnormalised captures, and retry tests inspect failing measurements rather than the precedence of diagnostic labels. A focused review caught and corrected a variable-shadowing mistake in the scale bracket before asset regeneration.

The lighting probe's wall time and the original parallel generation's end-to-end elapsed time were not recovered; model times are summed request wall times.

## Validation

All 12 mapped checks passed on `ef6fab404d494453bb88d385d2ddaac5dee3a328`: shell lint and offline transport fixtures, machine tests, retained-painter fixtures, Nix parsing, native/test and web/release builds, formatting, clippy with warnings denied, the full Rust suite, and the native/browser frame benchmark. The machine suite passed 22 tests plus its matching integration test. All four benchmark windows recorded zero over-budget frames (native maximum 6.083 ms; browser maximum 14.4 ms).

The final screenshot was rendered with lavapipe from the regenerated set and visually inspected. It loaded successfully without a shader error. The only subsequent changes are this validation note and the screenshot. The final credits read still reported `total_usage: 0`.
