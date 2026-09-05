# ziral art bible — Fired Workshop

The board is a tabletop instrument assembled from glazed ceramic, darkened brass, and soft rubber. Weight, wear, and one warm raking light make each action tactile; colored glazes keep every state unmistakable. Reference images and the prompts that made them are in [`reference/`](reference/).

The owner chose this direction on 2026-09-05 and set two rules, which section 3 makes checkable: every atom is visually distinct from every other atom, and the same holds for every glyph, every machine, and every bond type.

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

Hue is sRGB hue; luminance is linear relative luminance. Ivory has no hue worth naming (chroma 0.06); it reads by value only. Grout is dark brass at low alpha over clay, never a new color.

## 2. Language

**Materials and light.** Glazed clay for the board and atoms, rubbed brass for arms and bonds, matte rubber for hands. One warm light, raking from the upper left: a highlight sits high-left on every bead, wear gathers at contact edges only. No second light, no cast shadows that cross a cell boundary.

**Silhouette.** Stout circular pivots, one-piece links, an open horseshoe for the hand, hexagonal wells for glyph cells. Every state is readable from the outline alone: a closed hand is a small horseshoe on the atom, an open hand a wide one that fades; a stalled arm wears an ivory ring on its pivot, and when another hand is the cause that hand wears a wider one.

**Line.** Fills carry identity; lines carry structure. Grout and rims are thin dark brass. Glyph channels are the glyph's own glaze. Ivory lines mean "look here" (picked, stalled) and nothing else.

**Texture.** Glaze: one flat fill, one highlight, one darker rim. Brass: flat fill with a darker rim. No noise, no grain, no gradient that could be read as a state. Texture that survives at gameplay scale (a cell about forty pixels wide) is the only texture.

**Glyphs.** A glyph is an inset in the board: its cells are wells a shade darker than clay, and its channels are lines of its glaze joining slots and centre. A glyph never covers an atom; atoms sit in the wells.

**UI.** Dark brass strips with ivory text, brass borders; the lit tape row is the same strip in lighter brass. The strips are furniture, not board: nothing on them uses a state glaze.

**Motion.** Ticks have weighted starts and soft, decisive seats. What a glyph makes or eats appears at the end of the sweep, never mid-arc. Timing is a design knob (the tick period), not an art one.

**Forbidden.** Exposed clockwork; grime or wear over a state glaze; tiny ornamental hardware; text or numerals on the board; pure black or white (dark brass and ivory instead); any two members of one class told apart by hue alone; a new glaze for a new thing before the seven are used.

## 3. Distinctness

Two members of one class are distinct when they differ in at least two of hue, value, shape, marking, at gameplay scale. Hue counts only when both glazes have chroma at least 0.15 and their hues are at least 40° apart; value counts when luminance differs by at least 0.15; shape is the silhouette; marking is the inner drawing. Hue alone never counts, so the rule holds without color. The tests in `src/look.rs` check every pair in every class against exactly this rule, over the same table the toy draws from, so a new atom, glyph, machine, or bond that collides with an old one fails to land.

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
| source | blue-green | one-cell well | centre dot |
| bonder | terracotta | three-cell well, triangle | one spoke per slot; ring on the sacrificial slot |
| second bond | plum | three-cell well, triangle | two spokes per slot; ring on the sacrificial slot |
| output | ivory | two-cell well | a cup with a brass rim in each cell; the demanded bond stencilled between |

Every pair differs in at least two ways. The near cases: arm vs bonder share a hue family and value (brass 26° / 0.09 against terracotta 10° / 0.19) and are told apart by shape and marking; bonder vs second bond share a shape and are told apart by hue and marking.

## 4. Reference

`reference/sheet.png` is the presentation sheet; `concept-board.png`, `concept-arm.png`, `concept-glyph.png`, `texture-sheet.png` the concepts; `restyle-*.png` the real toy restyled, with `source-*.png` the shots they restyled. `prompts.txt` records the prompt and tool settings for each, so any image can be regenerated. `in-game.png` is the toy drawing this bible.
