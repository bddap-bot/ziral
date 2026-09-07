# ziral art bible — Fired Workshop

The board is a tabletop instrument assembled from glazed ceramic, darkened brass, and soft rubber. Weight and wear make each action tactile; colored glazes keep every state unmistakable. Reference images and the prompts that made them are in [`reference/`](reference/).

Two rules bind the direction, and section 3 makes them checkable: every atom is visually distinct from every other atom, and the same holds for every glyph, every machine, and every bond type.

## 1. Palette

Seven glazes. Every drawn thing takes its fill from this list; a new color is a bible change, not a code change.

| glaze | hex | hue | luminance | role |
|---|---|---|---|---|
| clay | `#D8C3A5` | 34° | 0.57 | the board; every empty cell |
| dark brass | `#6B4F3A` | 26° | 0.09 | arm pivot and link; single bond; the cleanup glyph; rims; grout; UI strips |
| terracotta | `#C8553D` | 10° | 0.19 | the hand; the bonder glyph |
| blue-green | `#4F8A8B` | 181° | 0.22 | the base atom; the source glyph |
| amber | `#E0A458` | 34° | 0.43 | reserved for the next atom kind |
| plum | `#7D5BA6` | 267° | 0.15 | the second-bond glyph; the double bond |
| ivory | `#F4EDE4` | — | 0.85 | the output cup; the pick and stall marks; UI text |

Hue is sRGB hue; luminance is linear relative luminance. Ivory has no hue worth naming (chroma 0.06); it reads by value only. Structure that is neither glaze nor state (grout, wells, rims, strips, borders) is dark brass lifted toward clay by some fraction, never a new color.

## 2. Language

**Materials and light.** Glazed clay for the board and atoms, rubbed brass for arms and bonds, matte rubber for hands. Rotatable art is captured as albedo under even, diffuse, non-directional light: curvature comes from symmetric tonal structure, wear gathers at contact edges only, and there is no cast shadow or directional highlight to swing with a turn. Machines alone carry relief: a normal map recovered from relit edits, lit at runtime by one fixed world light from the upper left, so their highlight stays on the world's light side through every turn.

**Silhouette.** Stout circular pivots, one-piece links, an open horseshoe for the hand, hexagonal wells for glyph cells. Every state is readable from the outline alone: a closed hand is a small horseshoe on the atom, an open hand a wide one; a stalled arm wears an ivory ring on its pivot, and when another hand is the cause that hand wears a wider one.

**Line.** Fills carry identity; lines carry structure. Grout and rims are thin dark brass. Glyph channels are the glyph's own glaze. Ivory lines mean "look here" (picked, being placed, stalled) and nothing else.

**Texture.** Every atom and bond wears a diffusion-generated macro of its material, while every machine is one complete top-down object sprite painted over a scaffold of its own footprint and kept in [`machines/`](machines/) with the manifest that remakes it (MACHINES.md). Atoms, bonds and tiles are kept in [`textures/`](textures/) with the prompt that made them. The board draws from twenty-four scaffolded clay hexes generated from one prompt, each cell's tile fixed by a hash of its coordinate, so one clay family carries handmade batch variation while its generated grout remains visible and its captured light stays fixed. A tile fills its cell, and its material crops the texture to the face the tool painted plus one stroke of grout, the face measured from the texture when the kiln fires, so a seam is two strokes of generated grout meeting and nothing else. Static seats, rims, markings, channels, and the open hand belong to the machine sprite rather than a second drawing over it; the live terracotta grip cue alone contracts over the open hand to show held state.

**Glyphs.** A glyph is a compact kiln-floor machine whose complete sprite spans its footprint. Its circular seats and functional channels remain broad and unobstructed beneath atoms, so the machine story never obscures how it is used.

**UI.** Dark brass strips with ivory numerals, brass borders; the lit tape row is the same strip in lighter brass. An instruction on a tape is a symbol token pressed into the strip, one per key, letter and action fused in one mark (art/SYMBOLS.md); a page of the workshop's own manual, held up while Tab is held, shows the seven tokens woven into flourishes beside a picture of what each does, and no writing (art/overlay/). The strips are furniture, not board: the strip itself never wears a state glaze; the tokens on it do, two glazes each, so they can be told apart at tape size.

**Motion.** Ticks have weighted starts and soft, decisive seats. What a glyph makes or eats appears at the end of the sweep, never mid-arc. Timing is a design knob (the tick period), not an art one.

**Forbidden.** Exposed clockwork; grime or wear over a state glaze; tiny ornamental hardware; text or numerals on the board; pure black or white (dark brass and ivory instead); any two members of one class told apart by hue alone; a new glaze for a new thing before the seven are used.

## 3. Distinctness

Two members of one class are distinct when they differ in at least two of hue, value, shape, marking, at gameplay scale. Hue counts only when both glazes have chroma at least 0.15 and their hues are at least 40° apart; value counts when luminance differs by at least 0.15; every row wears its own texture; shape is the silhouette; marking is the inner drawing. Hue alone never counts, so the rule holds without color. The tests in `src/look.rs` check every pair in every class against exactly this rule, over the same table the toy draws from, so a new atom, glyph, machine, or bond that collides with an old one fails to land, and a texture swap that makes two of them alike fails the same way. Atom and bond textures average to their glaze; each tile's interior stays inside one clay-family tolerance while every generated batch differs; machine sprites deliberately combine the bible materials.

The classes: atoms, bonds, glyphs, machines. A machine is a primitive the palette can place, so the machine class is the arm plus every glyph; glyph distinctness is the machine table restricted to glyphs.

### Atoms

| kind | glaze | shape | marking |
|---|---|---|---|
| base | blue-green | bead | one highlight, upper left |
| (next kind) | amber | bead | a brass band; differs from base in hue and marking |

### Bonds

| kind | glaze | shape | marking |
|---|---|---|---|
| single | dark brass | one bar | none |
| double | plum | two parallel bars | none |

Single and double differ in hue (26° vs 267°) and shape.

### Machines, including every glyph

| item | glaze | shape | marking |
|---|---|---|---|
| arm | dark brass, terracotta hand | radial: pivot disc and one link | horseshoe hand |
| source | blue-green | one-cell well | a ring and a centre dot |
| bonder | terracotta | two-cell well, bar | one spoke per slot; no ring |
| second bond | plum | three-cell well, triangle | two spokes per slot; ring on the sacrificial slot |
| output | ivory | two-cell well | a cup with a brass rim in each cell; the demanded bond stencilled between |
| cleanup | dark brass | one-cell well, sooted near-black | a brass ring around the void |

Every pair differs in at least two ways. The near cases: arm vs bonder share a hue family and value (brass 26° / 0.09 against terracotta 10° / 0.19) and are told apart by shape and marking; bonder vs second bond are told apart by hue, shape, and marking; bonder vs output share a footprint and are told apart by value and marking; source vs cleanup share a footprint and are told apart by hue and marking.

## 4. Reference

`reference/sheet.png` is the presentation sheet; `concept-board.png`, `concept-arm.png`, `concept-glyph.png`, `texture-sheet.png` the concepts; `restyle-*.png` the real toy restyled, with `source-*.png` the shots they restyled. `prompts.txt` records the prompt and tool settings for each, so any image can be regenerated. `in-game.png` is the toy drawing this bible. `paint.sh` is the one call to the image tool, with the style lock. `textures/` holds the atom, bond and tile textures with their `prompts.txt`, `gen.sh` to regenerate one (the tape symbols and the manual page are generated through it too), `sheet.png` the contact sheet, and `proof/` the toy wearing them, with `proof/grout.png`, a one-off from history: the seams before and after the tiles were cropped to their grout, at the default zoom and at 4x. `machines/` holds every machine as `albedo.png` and `normal.png` beside the scaffold, candidates, scores and relit edits that made them, `manifest.toml` the prompts and thresholds, `gen.sh` the pipeline, `sheet.png` every candidate with its scores, and `proof/` a machine at two turns under the one world light, plus `aspect.png`, a one-off from history: each machine's earlier sprite, squashed from a wider return, beside the shipped one at the default zoom, and `keyed.png`, another: a bonder and an arm over the board with their hex plates baked in beside the same scene with their backgrounds keyed out. `overlay/` holds the manual page held up on Tab: `gen.sh` composites the seven symbols onto `scaffold.png` at fixed slots and generates `manual.png` over it in edit mode, with the rejected generations under `candidates/`; `inputs.sha256` records the scaffold and prompt it was generated over, so a repainted symbol or a moved slot makes the page stale (`gen.sh -c` rebuilds the scaffold and says whether either has moved), `proof/slots.png` sets each symbol beside its slot on the page, `proof.sh` renders `proof/tab-held.png` and `proof/tab-released.png`. `symbols/` holds the tape symbol tokens the same way, with `candidates/` for the rejected generations, `proof.sh` rendering `proof/tape-26px.png`, the strip at tape size, and SYMBOLS.md the design of each. `sheet.sh` renders a contact sheet from the label and file pairs on its input. `arm-swing-curve.jpg` is the arm swing sketch, `arm-swing-curve-fit.png` the curve fitted to it, `arm-swing-strip.png` one tick of one arm, and `arm-personality.gif` five arms swinging the same tick, each with its own personality (the `chorus` shot scene), and `multiselect.gif` a marquee selection lifted, turned twice in hand and set down (the `select` shot scene), `multiselect-rotated.png` its still mid-hold, and `proof/pivot.gif` an arm holding a two-atom molecule running A E E D, the rotates sweeping about the pivot and the pivots about the hand (the `pivot` shot scene). `proof/cleanup-before.png` and `proof/cleanup-after.png` are an arm swinging the middle of a three-atom chain onto a cleanup glyph and the tick after, atom and glyph gone, the ends unbonded (the `cleanup` shot scene).
