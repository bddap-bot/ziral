# Tape symbols

Each instruction is one modular square generated from `art/symbols/source.svg`. `art/symbols/gen.sh` renders the thirteen 512 px assets without changing aspect, rebuilds `art/symbols/sheet.png`, and writes the shipped-size proof. The tape displays each asset at 26 px. The manual page held on Tab places those same runtime symbols over one reusable painted page.

Every display resolves the instruction through the same alpha-blended fired symbol texture, preserving the vector tile's transparent surround so its rounded corners are its visible edges at the shipped size. The tape, palette, shortage notice, hover card, pinned card, and manual add placement only.

The square is dark brass with a plum edge. Its bottom-left and top-right corners use a 28-unit radius; its top-left and bottom-right corners use a 96-unit radius. Every ivory key letter occupies the bottom-left and every ivory semantic mark occupies the top-right. Ivory on dark brass has a 6.44:1 WCAG-style luminance ratio from the palette's exact sRGB values, above the 4.5:1 floor, and every generated symbol is measured again through the runtime's mipmaps at the shipped 26 px. A lowercase letter means the key is pressed alone; an uppercase letter means Shift is held. Case is the only carrier of the shift state. These colors and positions do not vary by instruction.

The thirteen semantic marks are:

- f, grab: two inward points close on a bead. Closing around the bead is the plainest reading of taking hold.
- r, drop: an open point sits above a bead moving away. Separation is the plainest reading of release.
- a, counterclockwise rotate: a counterclockwise circular arrow. The action is rotation and nothing else moves independently.
- d, clockwise rotate: a clockwise circular arrow. It mirrors the counterclockwise action.
- q, counterclockwise pivot: a counterclockwise circular arrow around a fixed point. The point is the plainest distinction between turning the arm and turning about the hand.
- e, clockwise pivot: a clockwise circular arrow around a fixed point. It mirrors the counterclockwise pivot.
- x, wait: two vertical bars. The familiar pause mark is the plainest reading of one unchanged tick.
- Shift W, move upper-left: a straight upper-left arrow.
- Shift E, move upper-right: a straight upper-right arrow.
- Shift F, move right: a straight right arrow.
- Shift C, move lower-right: a straight lower-right arrow.
- Shift X, move lower-left: a straight lower-left arrow.
- Shift A, move left: a straight left arrow.

The six straight arrows are the instruction's board directions without a second metaphor. The generated letter keeps shortcuts readable while the mark makes repeated letters semantically distinct. The gates sample the actual 26 px render: every tile must retain the common palette, the two prescribed corner radii, visible letter and mark regions, and pairwise distinction.

`../proofs/instruction-contrast-86.png` places the full symbol set on a focused tape and in the palette at the shipped size, amber on plum before at left and ivory on dark brass after at right, without scaling either frame.

`../proofs/instruction-corners-87.png` places the tape, palette, shortage notice, hover card, pinned card, and runtime-composed manual in two unscaled shipped-size frames, with the surface beneath visible at every rounded corner.

# Inventory pips

Every inventory count uses one fixed 38 px circular footprint. Whole pips are concentric ivory rings from the centre outward. A fractional next pip is an ivory clockwise arc from the top over a brass ring. Hovering exposes brass rings through the cap. Palette cards and refusal cards use this one geometry; pinned cards and the source and output pictures add no second count geometry. The palette removes vertical card padding and uses a 1 px gap so every card remains inside the shipped viewport.

`../proofs/concentric-rings-113.png` compares the former marks with the shipped rings at native size on the same cards, including 1, 2½, 3¾ and the cap.
