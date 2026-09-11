# Tape symbols

Each instruction is one modular square generated from `art/symbols/source.svg`. `art/symbols/gen.sh` renders the thirteen 512 px assets without changing aspect, rebuilds `art/symbols/sheet.png`, and writes the shipped-size proof. The tape displays each asset at 26 px. The manual page held on Tab is regenerated over the same assets.

The square is plum with a dark-brass edge. Its bottom-left and top-right corners use a 28-unit radius; its top-left and bottom-right corners use a 96-unit radius. Every ivory key letter occupies the bottom-left and every amber semantic mark occupies the top-right. These colors and positions do not vary by instruction.

The thirteen semantic marks are:

- F, grab: two inward points close on a bead. Closing around the bead is the plainest reading of taking hold.
- R, drop: an open point sits above a bead moving away. Separation is the plainest reading of release.
- A, counterclockwise rotate: a counterclockwise circular arrow. The action is rotation and nothing else moves independently.
- D, clockwise rotate: a clockwise circular arrow. It mirrors the counterclockwise action.
- Q, counterclockwise pivot: a counterclockwise circular arrow around a fixed point. The point is the plainest distinction between turning the arm and turning about the hand.
- E, clockwise pivot: a clockwise circular arrow around a fixed point. It mirrors the counterclockwise pivot.
- X, wait: two vertical bars. The familiar pause mark is the plainest reading of one unchanged tick.
- Shift W, move upper-left: a straight upper-left arrow.
- Shift E, move upper-right: a straight upper-right arrow.
- Shift F, move right: a straight right arrow.
- Shift C, move lower-right: a straight lower-right arrow.
- Shift X, move lower-left: a straight lower-left arrow.
- Shift A, move left: a straight left arrow.

The six straight arrows are the instruction's board directions without a second metaphor. The generated letter keeps shortcuts readable while the mark makes repeated letters semantically distinct. The gates sample the actual 26 px render: every tile must retain the common palette, the two prescribed corner radii, visible letter and mark regions, and pairwise distinction.
