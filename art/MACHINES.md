# Machine textures

[BIBLE.md](BIBLE.md) offers state colours, visual direction and inspiration. [machines/manifest.toml](machines/manifest.toml) supplies directions, fixed thresholds and references. [src/machines.rs](../src/machines.rs) owns registration, measurement, selection and keying.

## Inputs and provenance

A glyph scaffold shows its footprint union in grey over green, with coloured rings and dots locating functional openings. These fills describe the envelope, not paint colours or a backing plate. The [body envelope guide](BIBLE.md#2-language) describes how the machine occupies that guide. Arm scaffolds and their separate shared direction retain the pivot, link and hand treatment.

The shared text, machine direction, Bible sections 1 and 2, scaffold and any manifest references reach `art/direct.sh`, which uses `art/ask.sh` to write a declarative image caption. The Bible sections are read when building each brief, including the palette and material-lighting rule. No recipe image or recipe sentence reaches the painter. The shared runner passes the [director model](director-model.txt) explicitly for both direction and judging, and rejects a missing transcript or any turn reporting another model. Painting uses the configured Codex installation.

Directive: “which uses `art/ask.sh` to write a declarative image caption”; “No recipe image or recipe sentence reaches the painter”.

`art/paint.sh` passes the caption unchanged to the image tool and verifies its actual call against that caption and the attached paths and SHA-256 hashes. Each prompt ends with an **Image inputs** section containing those verified inputs. Historical unrecoverable inputs remain explicit as `unknown`. A mismatched caption or attachment, implicit conversation-image reference or non-square return fails the paint. A square return is resized uniformly and quantised. Machine candidates' returned transparency is composited over the green key before measurement; this neither clips nor reshapes the object.

`prompt.txt` records the first round; `candidates/round-N.txt` records round N. These are evidence of their paints, not instructions to restore historical inputs. Regenerating after a shared-brief change authors new captions and paints new candidates; refreshing a hash alone is not repainting.

## Generate and select

Inside `nix-shell`, use `cargo run -- --gen bonder` or name multiple entries. `--gen --all` includes arms; a glyph-only change names the glyphs explicitly. Texture colours are repainted separately through `art/textures/gen.sh`.

1. **Register.** Seat rims supply one translation and one uniform scale. No stretch or rotation is applied.
2. **Measure.** `outside`, `seat` and `off_centre` are pipeline checks for footprint overflow, readable seats and registration. `palette` is a comparison measurement in the printed report and `scores.tsv`; it neither rejects nor ranks candidates. Distances use hex circumradii. The footprint measures the sprite; it never cuts its silhouette.
3. **Judge.** The critic sees the cut sprite on board tiles at gameplay scale, magnified without smoothing, beside its scaffold. The fixed rubric weighs overall visual strength, presence, material richness, detail and confidence. Detail counts in favour; bible prohibitions impose no automatic deductions, hard failures or score caps. The required `compound` boolean records whether the sprite reads as atoms joined by bonds; that reading informs the overall judgment and does not reject a candidate. Missing required reply fields fail judgment. Critic calls run sequentially.
   Historical direction, superseded by the bible’s no-hard-fail Directive:
   Directive: “Detail counts in favour”; “rejects any glyph that reads as a compound, regardless of score”.
4. **Repeat and keep.** A passing score of 8 ends painting early. Round two addresses the best candidate's measured or visual issues; round three starts from the brief again. At the three-round cap, keep the best measured, judged candidate, even below 8. A tie uses measured rank, then the earliest candidate. With none passing, generation fails. A critic that reads no measured passing candidate stops the run without another paint round.
   Directive: “A passing score of 8 ends painting early”; “At the three-round cap, keep the best”.
5. **Key.** Remove the green background into `albedo.png`, preserving painted alpha.

All rounds remain in one candidate directory; indices continue across rounds. `scores.tsv` includes measurements, critic score, rejection reason, issues, judgment key and compound flag. `sheet.png` displays candidates and the keep. A partially painted round is not topped up on resume. An unchanged candidate reuses its matching judgment; a judgment prompt or rubric change invalidates that cache during regeneration. Shipped prompts and score rows retain their original brief and judgment keys as historical evidence; changing guidance, including the bible palette the brief quotes, does not require repainting or rewriting those records.

The painter limits concurrent calls to six. Shell runners retry four times with doubling backoff and retain failure output.

## Cache keys

| Record | Inputs | Effect of a change |
|---|---|---|
| `briefed` | applicable shared text, direction, Bible sections 1 and 2, reference bytes | author a new caption |
| `painted` | first-round caption, candidate count, scaffold pixels and width, reference bytes | replace the candidate run |
| critic row | rubric, candidate bytes, judgment prompt | judge again |

An unchanged run remeasures its keep. A hand-edited caption under an unchanged brief is input to the next paint, not rewritten provenance. Atom, bond, tile and manual-page prompts remain adjacent to their assets; texture-generation reference conventions remain in their scripts and the bible.

A body without atom-seat landmarks registers from the complete keyed silhouette. Its bounding centre supplies one translation; its greatest hexagonal radius supplies one uniform scale into the complete footprint envelope. No silhouette pixel is clipped, and both axes share the scale. The pipeline checks and fixed critic judge that result.

Machines use their complete RGBA sprite. Whole-machine poses apply the pivot and motion from the simulation.

Directive: outputs do not need holes. An output is a space to place things — a magical acceptor, a table, or similar doodads — with Opus Magnum as the inspiration. No explicit acceptor hole for every atom.

Output briefs describe continuous receiving surfaces. Their scaffold cells describe body coverage, with no per-atom seat landmarks; their simulation cells and acceptance behavior are unchanged.
