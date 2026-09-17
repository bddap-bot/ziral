# Machine textures

[BIBLE.md](BIBLE.md) supplies the palette, view, footprint and distinctness rules. Each entry in [machines/manifest.toml](machines/manifest.toml) supplies a machine’s direction and any reference sketch. Those directions are inputs to the director, not copies of this document. The unresolved conflicts between directions, rubric and palette are listed on [issue #110](https://github.com/bddap-bot/ziral/issues/110).

## Inputs and provenance

A scaffold is a square image of functional seat marks on a flat green background. Blue-green rings mark seats that retain atoms; terracotta rings mark consumed inputs; brass marks an arm pivot; an open terracotta horseshoe marks its hand. Dots locate seat centres. Unmarked body cells contribute to the art footprint. The source is an exception: its five housing cells are filled amber in the scaffold around the central seat. Other scaffolds draw neither the body nor its connections nor a footprint outline. Its square corresponds to the renderer’s square, with a surrounding band removed before display.

The manifest’s shared text and per-machine direction form the brief. `art/direct.sh` sends that brief, the scaffold and any manifest references through `art/ask.sh` to a language model, which writes a declarative caption describing the finished picture. The director can change the wording and composition within the brief’s facts. The unresolved direction conflicts are not resolved by a precedence rule in the code. `ask.sh` uses the configured Codex model; the repository does not select a director model explicitly.

The caption becomes `machines/NAME/prompt.txt`. `art/paint.sh` sends its text unchanged to the image tool, then checks the recorded tool call in the session rollout. A mismatch between the supplied prompt and recorded tool call, or a non-square return, fails the paint; retries never stretch a return into shape. A source or cobalt-converter paint also receives its manifest reference sketch. No recipe image is rendered or attached.

Prompt files are paint provenance. Older machine prompts still mention recipe images or use imperative wording because those were the actual inputs to their paints. They are not current instructions to restore those inputs. `prompt.txt` records the first round; `candidates/round-N.txt` records round N, including the kept candidate’s round. Atom, bond, tile and manual-page prompts live beside their assets as `NAME.prompt.txt`; the concepts in `reference/` retain a historical `prompts.txt` log. Relight prompts are separate, as described below.

## Generate and select

[shell.nix](../shell.nix) supplies the build and image tools. Painting and judging also require an authenticated Codex installation configured for the intended model. [src/machines.rs](../src/machines.rs) defines scaffold dimensions, measured gates, critic reply validation and relief calculations.

From the repository root, inside `nix-shell`:

```sh
cargo run -- --gen NAME
cargo run -- --gen --all
```

Names are the machine and texture entry keys in the manifest. A machine entry remakes its candidate and relief stages; a texture entry relights the existing atom or bond texture. To repaint a texture, `art/textures/gen.sh` reads its adjacent prompt; pass any required reference with `-i`. Grout uses `art/textures/hex-scaffold.png`, tiles use `art/textures/grout.png`, and the base atom uses `art/reference/texture-sheet.png`. A replacement tile brief combines the palette and tile description in BIBLE.md. A complete repaint from the existing tile prompt is `art/textures/gen.sh -n 24 -i art/textures/grout.png tile`. To revise a texture’s direction, `art/direct.sh -i IMAGE BRIEF` prints the new prompt: `IMAGE` is the reference picture and `BRIEF` is the quoted finished-picture description using the bible’s material and palette. That output replaces the adjacent prompt for the next paint, not the historical round files. The manual page uses `art/paint.sh` with its adjacent prompt.

1. **Register.** The painted seat rims locate their centres. One shift and one uniform scale fit those centres to the scaffold; no stretch or turn is applied. The score, cut and relief use this registered capture.
2. **Measure.** Four manifest thresholds are hard gates: `outside` limits the farthest visible pixel beyond the art footprint, `seat` sets minimum seat-to-rim contrast, `palette` limits mean-colour distance from the nearest glaze, and `off_centre` limits seat displacement after registration. The two distances use hex circumradii.
3. **Judge.** For a measured passing candidate, `ask.sh` receives the cut sprite over board tiles at shipped size, magnified without smoothing, alongside the scaffold. The manifest rubric embeds a snapshot of earlier bible wording; it is not expanded from the current document at runtime. It ranks presence, material richness and artist moxie, then visual blemishes. The critic does not judge seat position, footprint or how seats connect. Its reply contains a score from 0 to 10 and at most five ranked issues. The rubric and threshold remain subject to [AGENTS.md](../AGENTS.md).
4. **Repeat and keep.** A score of 8 ends painting early. Otherwise, round two uses a rewritten prompt addressing the best candidate’s issues; round three starts from the brief again. At the three-round cap, the best measured passing candidate is kept even below 8, with its score and issues. An existing keep remains while it passes measurement and has a valid critic judgement. With no measured passing candidate, generation fails. If the critic reads no measured passing candidate, the run fails without another paint round.
5. **Cut.** The green background is removed into `albedo.png`, the colour image with transparency around the machine. The footprint judges the image; it does not cut its silhouette. `normal.png` stores surface directions for lighting, recovered by the relief stage.

Every round retains its candidates in one directory; indices continue across rounds. A tie in critic score is broken by measured rank (`seat - outside - palette`), then the earliest candidate. `scores.tsv` records the measurements, critic scores and issues; `sheet.png` shows the candidates and selection. A partially painted round is not topped up on resume.

The pipeline limits concurrent generation calls to six. The shell runners retry four times with doubling backoff and print failures. After generation, `art/machines/rig.sh` splits every machine with an albedo into a central circular moving part and its surrounding base, producing albedo, normal and emissive maps. This script does not interpret manifest masks or refresh per-part critic scores. The command fails if a requested entry or this split cannot complete; a failed contact-sheet render is diagnostic only.

## Cache keys

| Record | Inputs | Effect of a change |
|---|---|---|
| `briefed` | shared text, direction, reference-image bytes | director rewrites `prompt.txt` |
| `painted` | first-round prompt, candidate count, scaffold pixels and width, reference-image bytes | old candidates and scores are removed before a new run |
| critic row hash in `scores.tsv` | rubric and that candidate’s bytes | that candidate is judged again |
| `relit` | source pixels, relight brief, elevation, control facings and exact relight prompts | relief is regenerated |

A prompt edit under an unchanged brief remains the source for the next paint. Editing `kept` chooses an existing candidate through the same command; matching paint inputs preserve the candidate rounds. An unchanged run remeasures its keep and reuses a matching critic row. A stricter measured threshold does not erase a still-current critic score.

## Relief

The kept machine capture, or existing atom or bond texture, is centred without stretching inside the scaffold band. A grey calibration sphere is added to the band. The director writes each light’s exact prompt into `relit/prompt-FACING.txt`, and `paint.sh` paints over that master alone. The code measures each returned sphere’s light direction and ambient share, then recovers surface normals by least squares.

The manifest requests a warm light at 45° elevation and six directions around the image. The source uses four measured control lights, right, top, left and bottom, to derive its six-facing atlas. Other entries request all six directly. Both the sphere-shape error and the deviation from each requested light direction must stay within the unchanged 25° limit; a failed set is repainted, at most three sets. `relit/lights.txt` records the measured errors and directions.

Six-facing painter lighting is not connected to the shipped material. The first bounded run failed the light-aim or sphere-shape check for every class; [the attempt record](../proofs/relight-attempts-72.md) distinguishes those failures and the incomplete retained prompts. The source’s later four-control result does not establish a complete accepted set for every class. The shipped shader still lights the colour image through its normal map. The replacement design in [DESIGN.md](../DESIGN.md#rotatable-light) blends adjacent painter relights through turns only after the complete set passes.
