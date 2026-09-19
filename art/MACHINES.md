# Machine textures

[BIBLE.md](BIBLE.md) supplies the palette, view and footprint rules. [machines/manifest.toml](machines/manifest.toml) supplies directions, fixed thresholds and references. [src/machines.rs](../src/machines.rs) owns registration, measurement, selection and lighting.

## Inputs and provenance

A glyph scaffold shows its footprint union in grey over green, with coloured rings and dots locating functional openings. These fills describe the envelope, not paint colours or a backing plate. The finished machine is one substantial housing with apertures cut into it. Arm scaffolds and their separate shared direction retain the pivot, link and hand treatment.

The shared text, machine direction, scaffold and any manifest references reach `art/direct.sh`, which uses `art/ask.sh` to write a declarative image caption. No recipe image or recipe sentence reaches the painter. The shared runner passes the [director model](director-model.txt) explicitly for both direction and judging, and rejects a missing transcript or any turn reporting another model. Painting uses the configured Codex installation.

`art/paint.sh` passes the caption unchanged to the image tool and verifies its actual call against that caption and the attached paths and SHA-256 hashes. Each prompt ends with an **Image inputs** section containing those verified inputs. Historical unrecoverable inputs remain explicit as `unknown`. A mismatched caption or attachment, implicit conversation-image reference or non-square return fails the paint. A square return is resized uniformly and quantised. Machine candidates' returned transparency is composited over the green key before measurement; this neither clips nor reshapes the object.

`prompt.txt` records the first round; `candidates/round-N.txt` records round N. These are evidence of their paints, not instructions to restore historical inputs. Shared-brief changes require newly authored captions and newly painted candidates; refreshing a hash alone is not repainting.

## Generate and select

Inside `nix-shell`, use `cargo run -- --gen bonder` or name multiple entries. `--gen --all` includes arms and textures; a glyph-only change names the glyphs explicitly. Texture colours are repainted separately through `art/textures/gen.sh`; their relief command uses the existing colour image.

1. **Register.** Seat rims supply one translation and one uniform scale. No stretch or rotation is applied.
2. **Measure.** `outside`, `seat`, `palette` and `off_centre` are hard gates. Distances use hex circumradii. The footprint measures the sprite; it never cuts its silhouette.
3. **Judge.** The critic sees the cut sprite on board tiles at gameplay scale, magnified without smoothing, beside its scaffold. The fixed rubric weighs overall visual strength, presence, material richness, detail and confidence. Detail counts in favour; bible prohibitions impose no automatic deductions, hard failures or score caps. A separate required boolean rejects any glyph that reads as a compound, regardless of score. This separate housing gate remains independent of the holistic score. Arms are exempt. Missing required reply fields fail judgment. Critic calls run sequentially.
4. **Repeat and keep.** A passing score of 8 ends painting early. Round two addresses the best candidate's measured or visual issues; round three starts from the brief again. At the three-round cap, keep the best measured, judged, non-compound candidate, even below 8. A tie uses measured rank, then the earliest candidate. With none passing, generation fails. A critic that reads no measured passing candidate stops the run without another paint round.
5. **Cut and light.** Remove the key into `albedo.png`, derive `normal.png`, compute the six calibration relights, and split only the requested machines into rig parts.

All rounds remain in one candidate directory; indices continue across rounds. `scores.tsv` includes measurements, critic score, rejection reason, issues, judgment key and compound flag. `sheet.png` displays candidates and the keep. A partially painted round is not topped up on resume. An unchanged candidate reuses its matching judgment; a glyph gate or rubric change invalidates that cache.

The painter limits concurrent calls to six. Shell runners retry four times with doubling backoff and retain failure output. `art/machines/rig.sh` splits selected machines into a central circular moving part and its surrounding base, with colour, normal and emissive maps. It accepts entry names; an omitted list selects all entries. It does not interpret manifest masks or refresh per-part taste scores.

## Relief

The generator paints albedo only. No generated relight or direction prompt is used. A deterministic shallow height approximation combines the silhouette with 15% luminance variation, blurred by four capture pixels. Central differences produce tangent-space normals. This is approximate relief, not recovered physical geometry: colour changes can contribute small bumps. The kept colour supplies ceramic, brass and rubber detail.

One renderer shades both machine and analytic grey calibration sphere with `ambient + (1 - ambient) * max(normal · light, 0)`. Every light uses the manifest elevation and facing directly: six directions at 45° elevation. The sphere is part of that same render, never a correction pasted onto a generated relight. Directions and sphere-shape error are measured from the pixels with the unchanged 25° limit. Quantisation can leave a small measured direction error despite exact input directions.

`relit/master.png` is the unlit colour and sphere. The six named PNGs are computed renders; `lights.txt` records requested and measured directions, angular errors and ambient share. `relit/albedo.png` stacks the six cut renders. The shipped material continues to light the colour image through `normal.png`; the atlas is calibration evidence, not a second runtime material. Old generated-relight attempts in `proofs/` are historical failed evidence.

## Cache keys

| Record | Inputs | Effect of a change |
|---|---|---|
| `briefed` | applicable shared text, direction, reference bytes | author a new caption |
| `painted` | first-round caption, candidate count, scaffold pixels and width, reference bytes | replace the candidate run |
| critic row | rubric, candidate bytes, glyph gate | judge again |
| `relit` | source pixels, relief algorithm, elevation, facings, ambient share | recompute relief |

An unchanged run remeasures its keep. A hand-edited caption under an unchanged brief is input to the next paint, not rewritten provenance. Atom, bond, tile and manual-page prompts remain adjacent to their assets; texture-generation reference conventions remain in their scripts and the bible.

A body without atom-seat landmarks registers from the complete keyed silhouette. Its bounding centre supplies one translation; its greatest hexagonal radius supplies one uniform scale into the single-cell envelope. No silhouette pixel is clipped, and both axes share the scale. The ordinary measured gates and fixed critic judge that result.

Static housings keep an empty part list and use the complete albedo. Only articulated machines carry split maps and part judgments; the portal has neither a firing rim nor a recorded part score.
