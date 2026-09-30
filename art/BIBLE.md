# ziral art bible — Fired Workshop

Directive: “Fired Workshop”.

The current portal is one substantial plum ceramic housing within one tile, with a broad brass and ivory aperture and mineral depth inside. The canonical interior preview uses the ordinary scene renderer inside that opening. Interior tiles use a separately painted ethereal plum and ivory mineral surface with the same grout geometry; overworld tiles retain the clay batch. This is a starting point for new frames, including organic materials and asymmetric openings.

Directive: “within one tile”.

A completed portal click briefly dilates the complete housing uniformly and releases ivory steam above its aperture, accompanied by its ceramic instrument. The canonical preview remains still; the response lasts one 400 ms tick independently of the overworld tick boundary.

The board is a tabletop instrument assembled from glazed ceramic, darkened brass, and soft rubber. Weight and wear make each action tactile. The concept art in [`reference/`](reference/) offers inspiration and ideas to explore freely.

[MACHINES.md](MACHINES.md) describes the current painting pipeline; [SYMBOLS.md](SYMBOLS.md) describes instruction tokens and inventory pips. [DESIGN.md](../DESIGN.md) records decisions and experiments, including superseded art. Prompt files record actual paints, not current rules. Each ends with an Image inputs section listing repository paths and SHA-256 hashes from the same verified image tool call as the caption; older records mark unrecoverable inputs unknown. The section is provenance and is excluded when reusing the caption.

## 1. Palette

Eight glazes identify states; material and ornament colours are free. The table describes the current palette, including one rubber material.

| colour | hex | hue | luminance | role |
|---|---|---|---|---|
| clay | `#D8C3A5` | 34° | 0.57 | the board; every empty cell |
| dark brass | `#6B4F3A` | 26° | 0.09 | arm pivot and link; single bond; rims; grout; UI strips |
| terracotta | `#C8553D` | 10° | 0.19 | the hand collar; the bonder glyph |
| blue-green | `#4F8A8B` | 181° | 0.22 | the base atom; the source glyph |
| amber | `#E0A458` | 34° | 0.43 | the amber atom; saturated gold |
| plum | `#7D5BA6` | 267° | 0.15 | the second-bond glyph; the double bond |
| cobalt | `#3657A7` | 220° | 0.10 | the cobalt atom; its converter path |
| ivory | `#F4EDE4` | — | 0.85 | the output cup; the pick and stall marks; instruction letters |
| charcoal rubber | `#423B37` | 22° | 0.05 | matte hand rubber; material colour, never state |

Directive: the arm brief is not authored taste and may be changed artistically; the arm and hand as painted are approved. No rigid reading of the eight-glaze rule against them.

Hue is sRGB hue; luminance is linear relative luminance. Ivory reads chiefly by value.

## 2. Language

**Materials and light.** Glazed clay, rubbed brass and matte rubber offer a tactile starting point; other materials are welcome. Technical note: capture albedo under even, diffuse light because runtime normal-map lighting supplies direction; baked highlights and shadows would compete with it. [MACHINES.md](MACHINES.md#relief) describes the relief approximation.

Directive: “Every machine body, including arms, has rich material: glaze variation, shading and depth are expected”; “The complete arm creeps, overshoots, and rings out”.

Fittings are welcome and need not do visible work: ports, plates, rivets, pipes and toothed collars.

Directive: the ornamental-hardware ban is rejected. Ornamental hardware is wanted; lean into the look — ports, plates, rivets, pipes, toothed collars — and let the art be generous with it.

**View.** Tonal asymmetry alone cannot distinguish a tilted cup from a shaded straight-down one; judge the whole image.

Directive: “Every machine is drawn from directly above, the camera straight down and orthographic”.

**Footprint.** Technical note: the pipeline measures overflow against the art footprint so neighbouring pieces can remain legible. An arm’s art footprint includes its pivot, crossed cells and hand; gameplay placement occupies only its pivot. [MACHINES.md](MACHINES.md#inputs-and-provenance) describes the scaffold and tolerance.

Directive: “No pixel left visible after removal of the green background extends beyond the union of the art footprint’s hexes”.

**Body envelope.** [The manifest](machines/manifest.toml) records the current painting briefs and body coverage guides.

**Silhouette.**

Directive: “complete machine housings around glyph seats”.

**Line.** Fills carry identity; lines carry structure.

**Texture.** Current sprites, macros and prompts live in [`machines/`](machines/) and [`textures/`](textures/). Technical note: the renderer gives all tiles the same grout pixels to avoid seams between independently painted borders. Tile faces can explore handmade variation; the template supplies their shared edge.

Directive: “Every atom and bond wears a diffusion-generated macro of its material”; “Atoms, bonds and tiles are kept in [`textures/`](textures/), each with the prompt that painted it beside it”; “one clay family carries handmade batch variation”; “its generated grout remains visible”; “Grout is one image, `textures/grout.png`: the scaffold every tile is painted over”.

**Glyphs.** Clear atom seats help atoms read against their housing. Separate round nodes joined by narrow necks or rails can suggest a compound; weigh that reading with the overall visual strength and detail.

Directive: “A glyph never reads as a compound”; “Arms retain their distinct pivot, link and hand silhouette”.

**UI.** The current interface uses brass framing and clay fields; [SYMBOLS.md](SYMBOLS.md) describes instruction pictures and inventory pips.

Directive: “an atom is the board’s circular bead with its brass rim”; “the item, its recipe in atoms and bonds, and fixture playback on the board’s clay”; “Cards carry no writing apart from the key letters inside instruction pictures”; “Tab holds up the painted manual page with those same instruction pictures and no other writing”; “the bead within its circular body, or the machine at the same cell outside it”.

**Ghost.** The ivory tally distinguishes projected frames while their materials retain the same light and opacity.

**Motion.** Weighted starts and soft, decisive seats can make ticks tactile. The current sweep resolves in 400 ms.

Directive: “a stall is backpressure”.

**Forbidden.**

Directive: “text or numerals on the board”.

Directive: no hard fail on the bible’s prohibitions. The judge weighs overall visual strength and detail; the more detailed candidates are preferred. Artistic constraints inform judgment rather than impose score caps.

## 3. Distinctness

Machines should read as distinct from each other.

Directive: “The classes: atoms, bonds, glyphs, machines”.

These tables describe current painted features as examples, not requirements for future art.

### Atoms

| kind | glaze | shape | marking |
|---|---|---|---|
| base | blue-green | bead | a brass band and captured highlight |
| amber | amber | ringed bead | a brass band |
| plum | plum | faceted bead | an ivory inset |
| cobalt | cobalt | knobbed bead | six broad knobs around a brass centre |

### Bonds

| kind | glaze | shape | marking |
|---|---|---|---|
| single | dark brass | one bar | none |
| double | plum | two parallel bars | none |

### Machines, including every glyph

| item | glaze | shape | marking |
|---|---|---|---|
| arm, length one | dark brass, terracotta hand | radial: pivot disc and one-cell link | hand |
| arm, length two | dark brass, blue-green inlay, terracotta hand | radial: pivot disc and two-cell link | hand |
| arm, length three | dark brass, blue-green inlays, terracotta hand | radial: pivot disc and three-cell link | hand |
| portal | plum | one-cell housing | ivory and brass aperture with mineral depth |
| source, first tier | blue-green | six-cell housing open to the right | five feeds around one central outlet |
| source, second tier | blue-green | six-cell housing in two staggered columns | two side-by-side outlets |
| bonder | terracotta | two-cell compression housing | equal seats and a brass compression channel |
| second bond | plum | three-cell manifold | two bond seats and a distinct sacrificial feed |
| amber converter | amber | three-cell fork | three brass-ringed seats and a Y channel |
| resonator | plum | two-cell housing | equal amber-lined seats and a shared brass resonant crest |
| cobalt converter | cobalt | four-cell rhombus | opposite input and output openings; a cobalt path through two body cells |
| reification | amber | nineteen-cell hexagon | two rings of seats and channels around a central receiving cup |
| output, first tier | ivory | seven-cell well, hexagon | seven cups with brass rims, rails from the centre cup |
| output, second tier | ivory | nineteen-cell well, hexagon | the first tier ringed by twelve cups and a brass ring |
| output, third tier | ivory | thirty-seven-cell well, hexagon | the second tier ringed by eighteen cups and a second brass ring |

Arm reach and output capacity offer useful visual differences to explore.

Directive: “The output tiers have seven, nineteen and thirty-seven seats”.

## 4. Reference

The concept art in [reference/](reference/) is an invitation to explore the workshop’s materials, forms and ornament. Historical comparisons and gameplay captures live in `../proofs/`.

Asset directories, relative to this document:

- [reference/](reference/) contains concepts, their historical prompt log and gameplay proofs. These are references rather than shipped textures.
- [textures/](textures/) contains atoms, bonds, the grout template and twenty-four tiles beside their paint prompts. The template originates in hex-scaffold.svg; all tiles use the template’s grout at runtime.
- [machines/](machines/) contains each machine’s caption, attempt record, albedo and normal map. [MACHINES.md](MACHINES.md) explains their provenance and regeneration.
- [overlay/](overlay/) contains the painted manual page; runtime composition places the instruction pictures in its slots.
- [symbols/](symbols/) contains the vector source and generated instruction pictures described in [SYMBOLS.md](SYMBOLS.md).

The three output cards play the shared fixtures in [src/sim.rs](../src/sim.rs) receiving the bonder, second-bond and amber-converter compounds respectively; their own recipes remain in the recipe panel. `../proofs/output-products-112-3823.png` shows all three cards at native size.

Directive: “their own recipes remain in the recipe panel”.

A portal's aperture frames the canonical extent through one uniform transform. Its housing fades as the camera fills that aperture. A body with no atom-seat marks registers by its complete silhouette with one translation and uniform scale. Registration preserves the complete silhouette. The ethereal texture uses the grout template's square resolution as its size source. Its first painting is retained at critic score 8 in `textures/ethereal-candidates/`.
