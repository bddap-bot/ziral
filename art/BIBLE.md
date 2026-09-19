# ziral art bible — Fired Workshop

The portal is one substantial plum ceramic housing within one tile, with a broad brass and ivory aperture and mineral depth inside. The canonical interior preview uses the ordinary scene renderer inside that opening. Interior tiles use a separately painted ethereal plum and ivory mineral surface with the same grout geometry; overworld tiles retain the clay batch. Machine materials and the ivory replay tally remain unchanged.

A completed portal click briefly dilates the complete housing uniformly and releases ivory steam above its aperture, accompanied by its ceramic instrument. The canonical preview remains still; the response lasts one 400 ms tick independently of the overworld tick boundary.

The board is a tabletop instrument assembled from glazed ceramic, darkened brass, and soft rubber. Weight and wear make each action tactile; colored glazes keep every state unmistakable. Reference images and the prompts that made them are in [`reference/`](reference/).

[MACHINES.md](MACHINES.md) describes the current painting pipeline; [SYMBOLS.md](SYMBOLS.md) describes instruction tokens and inventory pips. [DESIGN.md](../DESIGN.md) records decisions and experiments, including superseded art. Prompt files record actual paints, not current rules. Each ends with an Image inputs section listing repository paths and SHA-256 hashes from the same verified image tool call as the caption; older records mark unrecoverable inputs unknown. The section is provenance and is excluded when reusing the caption.

## 1. Palette

Eight state glazes and one material colour. Every drawn thing takes its fill from this table; a new color is a bible change, not a code change.

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

Hue is sRGB hue; luminance is linear relative luminance. Ivory has no hue worth naming (chroma 0.06); it reads by value only. The amber atom loses no more than 0.15 chroma from its glaze, keeping its gold saturated through the surface variation. Other structure that is neither glaze nor state (grout, wells, rims, strips, borders) is dark brass lifted toward clay by some fraction, never a new color.

## 2. Language

**Materials and light.** Glazed clay for the board and atoms, rubbed brass for arms and bonds, matte rubber for hands. The shipped material lights albedo through normal maps; activation adds a ripple and flare. Machine relief is a deterministic shallow approximation from colour and silhouette. Six calibration relights use explicit light vectors and the same computed shading for machine and sphere, described in [MACHINES.md](MACHINES.md#relief). The complete arm creeps, overshoots, and rings out in one rotation about its pivot; its painted parts add no second turn.

Fittings are welcome and need not do visible work: ports, plates, rivets, pipes and toothed collars.

Directive: the ornamental-hardware ban is rejected. Ornamental hardware is wanted; lean into the look — ports, plates, rivets, pipes, toothed collars — and let the art be generous with it.

**View.** Every machine is drawn from directly above, the camera straight down and orthographic. Visible side faces, a cup’s far inner wall and vanishing points are judged by the image critic: tonal asymmetry alone cannot distinguish a tilted cup from a shaded straight-down one.

**Footprint.** No pixel left visible after removal of the green background extends beyond the union of the art footprint’s hexes by more than the manifest’s `outside` share of a hex circumradius (centre to corner). The art footprint includes an arm’s pivot, crossed cells and hand; its gameplay placement occupies only the pivot. Glyph art includes both functional seats and body cells. [MACHINES.md](MACHINES.md#inputs-and-provenance) describes the scaffold, including the source’s painted housing cells.

**Silhouette.** Stout circular pivots, one-piece links, an open horseshoe for the hand, and complete machine housings around glyph seats. Every state is readable from the outline alone: a closed hand is a small horseshoe on the atom, an open hand a wide one; a stalled arm wears an ivory ring on its pivot, and when another hand is the cause that hand wears a wider one.

**Line.** Fills carry identity; lines carry structure. Grout and rims are thin dark brass. Glyph channels are the glyph's own glaze. Ivory board outlines mark selection, placement and stalls.

**Texture.** Every atom and bond wears a diffusion-generated macro of its material, while every machine is one complete top-down object sprite painted over a scaffold marking its functional seats and kept in [`machines/`](machines/) with the manifest that remakes it (MACHINES.md). Atoms, bonds and tiles are kept in [`textures/`](textures/), each with the prompt that painted it beside it. The board draws from twenty-four scaffolded clay hexes generated from one prompt, each cell's tile fixed by a hash of its coordinate, so one clay family carries handmade batch variation while its generated grout remains visible and its captured light stays fixed. Grout is one image, `textures/grout.png`: the scaffold every tile is painted over, its surround generated once as coarse sanded grout to the image edge. At texture loading, the renderer lays the template's pixels over everything beyond the scaffold stroke's inner edge and crops at the stroke's outer edge, so every tile's ring is the same pixels and a seam is one grout meeting itself; a new tile is painted over the template so its clay reaches the ring, it is never judged on its border, and a template must keep its grain on every edge at the shipped size. Static seats, rims, markings, channels, and the open hand belong to the machine sprite rather than a second drawing over it; the live terracotta grip cue alone contracts over the open hand to show held state.

**Glyphs.** A glyph is one substantial built housing covering its footprint, with circular seats cut into its continuous body as broad, unobstructed openings. It never reads as a compound: separate round nodes joined by bonds, narrow necks or rails fail regardless of numeric critic score. Arms retain their distinct pivot, link and hand silhouette.

**UI.** Dark brass strips and borders frame the inventory and tape; the lit tape row is lighter brass. Each machine or atom picture sits on a circular clay field; an atom is the board’s circular bead with its brass rim. Instruction tokens sit directly on the strip. [SYMBOLS.md](SYMBOLS.md) defines their geometry and the inventory counts. Hover and pinned cards share a clay surface within one brass border, with the item, its recipe in atoms and bonds, and fixture playback on the board’s clay. Cards carry no writing apart from the key letters inside instruction pictures. Tab holds up the painted manual page with those same instruction pictures and no other writing. On the board, an ivory hover outline surrounds the target a press would take: the bead within its circular body, or the machine at the same cell outside it.

**Ghost.** A ghost is a projected frame drawn alone while the canonical frame waits. The existing ivory tally is its only cue: G adds a mark, S removes one, and the canonical frame has none. Machines retain their lit material and activation response; atoms, bonds, rims and lines retain their normal opacity. The projection does not alter the interior tile treatment.

**Motion.** Ticks have weighted starts and soft, decisive seats. What a glyph makes or eats appears at the end of the sweep, never mid-arc. The tick is 400 ms; a stall is backpressure.

**Forbidden.** Exposed clockwork; grime or wear over a state glaze; text or numerals on the board; pure black or white (dark brass or charcoal rubber, and ivory instead); any two members of one class told apart by hue alone; a new glaze for a new thing before the existing palette is used.

## 3. Distinctness

[The look table and its tests](../src/look.rs) define chroma, texture averaging, thumbnail comparison and tile tolerances. [The machine generator](../src/machines.rs) defines registration, alpha removal, contrast and footprint measurements. These links provide the exact metrics behind the prose.

The visual requirement is two differences among hue, value, silhouette and inner drawing at gameplay scale, with hue alone insufficient. The mechanical gate in src/look.rs counts hue, value, declared shape category and painted texture difference. It does not measure the rendered silhouette or recognize a semantic marking, and passing it alone does not prove color-independent readability.

Hue counts when both glazes have chroma at least 0.15 and their hues differ by at least 40°; value counts at a luminance difference of 0.15. The texture comparison uses a sixteen-pixel thumbnail over pixels painted by either sprite, excluding their shared empty surround. Every member of a class uses a different texture. Atom and bond textures average to their glaze; tiles stay within the clay-family tolerance while differing from each other; machine sprites combine the bible materials. Reconciliation of the visual requirement and its mechanical approximation remains in the [review decision list](https://github.com/bddap-bot/ziral/issues/110).

The classes: atoms, bonds, glyphs, machines. A machine is a primitive the palette can place, so the machine class is the arm plus every glyph; glyph distinctness is the machine table restricted to glyphs.

The tables describe painted features; the marking column is descriptive, not a fifth gate. Atom meshes remain circular even when their declared shape categories differ.

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

Single and double differ in hue (26° vs 267°) and shape.

### Machines, including every glyph

| item | glaze | shape | marking |
|---|---|---|---|
| arm, length one | dark brass, terracotta hand | radial: pivot disc and one-cell link | horseshoe hand |
| arm, length two | dark brass, blue-green inlay, terracotta hand | radial: pivot disc and two-cell link | horseshoe hand |
| arm, length three | dark brass, blue-green inlays, terracotta hand | radial: pivot disc and three-cell link | horseshoe hand |
| portal | plum | one-cell housing | square ivory and brass aperture with mineral depth |
| source | blue-green | six-cell housing open to the right | five feeds around one central outlet |
| bonder | terracotta | two-cell compression housing | equal seats and a brass compression channel |
| second bond | plum | three-cell manifold | two bond seats and a distinct sacrificial feed |
| amber converter | amber | three-cell fork | three brass-ringed seats and a Y channel |
| plum converter | plum | three-cell bend | one amber seat, two plum seats and paired rails |
| cobalt converter | cobalt | four-cell rhombus | opposite input and output openings; a cobalt path through two body cells |
| reification | amber | nineteen-cell hexagon | two rings of seats and channels around a central receiving cup |
| output, first tier | ivory | seven-cell well, hexagon | seven cups with brass rims, rails from the centre cup |
| output, second tier | ivory | nineteen-cell well, hexagon | the first tier ringed by twelve cups and a brass ring |
| output, third tier | ivory | thirty-seven-cell well, hexagon | the second tier ringed by eighteen cups and a second brass ring |

The gate counts at least two differences per pair. The near cases: the three arm lengths share hue, value and marking and differ by link length and texture; each arm vs bonder shares a hue family and value (brass 26° / 0.09 against terracotta 10° / 0.19) and is told apart by shape and texture; bonder vs second bond are told apart by hue, shape, and texture; the three output tiers share glaze and value and are told apart by size, seven, nineteen and thirty-seven cells, and by texture; the texture term is judged on a sixteen-pixel thumbnail of each sprite, since at eight the second and third tiers, whose cups differ two to one in size, read as one ivory blur.

## 4. Reference

`../proofs/amber-63.png` places the base, amber, and plum atoms side by side in the world at shipped size after the amber texture's saturation regrade.

`../proofs/machine-coverage-67.png` places three machine-coverage proposals on the same real board crop at shipped size: broad rounded housings, bodies masked to occupied hexes, and seat-following bodies bounded by a brass rim. It is a comparison, not a change to the footprint rules above.

`../proofs/machine-body-first-67.png` places new body-first arm, bonder and first-output proposals on the real board at shipped size. Each direction starts from one cast housing with its functional marks cut into the body; the sheet is a comparison and does not replace shipped machine art.

`../proofs/arm-cobalt-recipe-101.png` is the shipped arm card at its native 405 by 184 pixels, with the five cobalt atoms and four single bonds drawn at tape size without stretching.

`../proofs/arm-lengths-102.gif` places the three arm lengths on one tape at the shipped size, each grabbing, rotating and dropping at its own reach.

`../proofs/view-sound-58.mp4` moves the shipped view from one running side of the board to the other while the tick mix follows the visible cells and zoom.

`../proofs/machine-drag-88.gif` is one machine lifted clear of its origin and carried continuously under the pointer at the shipped size.

`../proofs/free-debug-stepping-79.gif` is the `ghost` shot scene at shipped size, paused and stepped five frames forward and three back without a step row or count.

Asset directories, relative to this document:

- [reference/](reference/) contains concepts, their historical prompt log and gameplay proofs. These are references rather than shipped textures.
- [textures/](textures/) contains atoms, bonds, the grout template and twenty-four tiles beside their paint prompts. The template originates in hex-scaffold.svg; all tiles use the template’s grout at runtime.
- [machines/](machines/) contains each machine’s scaffold, paint prompts, candidates, scores, albedo, normal map and relight evidence. [MACHINES.md](MACHINES.md) explains their provenance and regeneration.
- [overlay/](overlay/) contains the painted manual page; runtime composition places the instruction pictures in its slots.
- [symbols/](symbols/) contains the vector source and generated instruction pictures described in [SYMBOLS.md](SYMBOLS.md).

The three output cards play the shared fixtures in [src/sim.rs](../src/sim.rs) receiving the bonder, second-bond and amber-converter compounds respectively; their own recipes remain in the recipe panel. `../proofs/output-products-112-3823.png` shows all three cards at native size.

A portal's square aperture frames the canonical extent through one uniform transform. Its housing fades as the camera fills that aperture. A body with no atom-seat marks registers by its complete silhouette with one translation and uniform scale; the silhouette is never cut to its footprint. The ethereal texture uses the grout template's square resolution as its size source. Its first painting is retained at critic score 8 in `textures/ethereal-candidates/`.
