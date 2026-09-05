# ziral art bible — Fired Workshop

The board is a tabletop instrument assembled from glazed ceramic, darkened brass, and soft rubber. Weight, wear, and one warm raking light make each action tactile; colored glazes keep every state unmistakable. Reference images and the prompts that made them are in [`reference/`](reference/).

Two rules bind the direction, and section 3 makes them checkable: every atom is visually distinct from every other atom, and the same holds for every glyph, every machine, and every bond type.

## 1. Palette

Seven glazes. Every drawn thing takes its fill from this list; a new color is a bible change, not a code change.

| glaze | hex | hue | luminance | role |
|---|---|---|---|---|
| clay | `#D8C3A5` | 34° | 0.57 | the board; every empty cell |
| dark brass | `#6B4F3A` | 26° | 0.09 | arm pivot and link; single bond; rims; grout; UI strips |
| terracotta | `#C8553D` | 10° | 0.19 | the hand; the bonder glyph |
| blue-green | `#4F8A8B` | 181° | 0.22 | the base atom; the source glyph |
| amber | `#E0A458` | 34° | 0.43 | reserved for the next atom kind |
| plum | `#7D5BA6` | 267° | 0.15 | the second-bond glyph; the double bond |
| ivory | `#F4EDE4` | — | 0.85 | the output cup; the pick and stall marks; UI text |

Hue is sRGB hue; luminance is linear relative luminance. Ivory has no hue worth naming (chroma 0.06); it reads by value only. Structure that is neither glaze nor state (grout, wells, rims, strips, borders) is dark brass lifted toward clay by some fraction, never a new color.

## 2. Language

**Materials and light.** Glazed clay for the board and atoms, rubbed brass for arms and bonds, matte rubber for hands. One warm light, raking from the upper left: a highlight sits high-left on every bead, wear gathers at contact edges only. No second light, no cast shadows that cross a cell boundary.

**Silhouette.** Stout circular pivots, one-piece links, an open horseshoe for the hand, hexagonal wells for glyph cells. Every state is readable from the outline alone: a closed hand is a small horseshoe on the atom, an open hand a wide one; a stalled arm wears an ivory ring on its pivot, and when another hand is the cause that hand wears a wider one.

**Line.** Fills carry identity; lines carry structure. Grout and rims are thin dark brass. Glyph channels are the glyph's own glaze. Ivory lines mean "look here" (picked, being placed, stalled) and nothing else.

**Texture.** Every surface wears a diffusion-generated macro of its material, kept in [`textures/`](textures/) with the prompt that made it: each machine, atom kind, bond kind, and glyph kind has its own, and the board draws from twenty-four clay tiles, each cell's tile and turn fixed by a hash of its coordinate, so the variance from tile to tile is visible and still. A texture is a surface, never a state: rims, markings, and lines stay drawn fills over it, and nothing in a texture may read as a state glaze at gameplay scale (a cell about forty pixels wide).

**Glyphs.** A glyph is an inset in the board: its cells are wells floored in the glyph's own glaze texture, and its channels are bars of its glaze joining slots and centre. A glyph never covers an atom; atoms sit in the wells, and everything of a glyph is drawn beneath them.

**UI.** Dark brass strips with ivory text, brass borders, the readout on its own strip; the lit tape row is the same strip in lighter brass. The strips are furniture, not board: nothing on them uses a state glaze.

**Motion.** Ticks have weighted starts and soft, decisive seats. What a glyph makes or eats appears at the end of the sweep, never mid-arc. Timing is a design knob (the tick period), not an art one.

**Forbidden.** Exposed clockwork; grime or wear over a state glaze; tiny ornamental hardware; text or numerals on the board; pure black or white (dark brass and ivory instead); any two members of one class told apart by hue alone; a new glaze for a new thing before the seven are used.

## 3. Distinctness

Two members of one class are distinct when they differ in at least two of hue, value, shape, marking, at gameplay scale. Hue counts only when both glazes have chroma at least 0.15 and their hues are at least 40° apart; value counts when luminance differs by at least 0.15; both are judged on the glaze and on the mean colour of the texture together, and count only when both differ; every row wears its own texture; shape is the silhouette; marking is the inner drawing. Hue alone never counts, so the rule holds without color. The tests in `src/look.rs` check every pair in every class against exactly this rule, over the same table the toy draws from, so a new atom, glyph, machine, or bond that collides with an old one fails to land, and a texture swap that makes two of them alike fails the same way. A texture also has to average to its own glaze, and every tile has to differ from every other tile at gameplay scale.

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

Every pair differs in at least two ways. The near cases: arm vs bonder share a hue family and value (brass 26° / 0.09 against terracotta 10° / 0.19) and are told apart by shape and marking; bonder vs second bond are told apart by hue, shape, and marking; bonder vs output share a footprint and are told apart by value and marking.

## 4. Reference

`reference/sheet.png` is the presentation sheet; `concept-board.png`, `concept-arm.png`, `concept-glyph.png`, `texture-sheet.png` the concepts; `restyle-*.png` the real toy restyled, with `source-*.png` the shots they restyled. `prompts.txt` records the prompt and tool settings for each, so any image can be regenerated. `in-game.png` is the toy drawing this bible. `textures/` holds every surface texture with its own `prompts.txt`, `gen.sh` to regenerate one, `sheet.png` the contact sheet, and `proof/` the toy wearing them. `arm-swing-curve.jpg` is the arm swing sketch, `arm-swing-curve-fit.png` the curve fitted to it, `arm-swing-strip.png` one tick of one arm, and `arm-personality.gif` five arms swinging the same tick, each with its own personality (the `chorus` shot scene).
