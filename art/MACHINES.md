# Machine bible — Fired Workshop

The machines are compact kiln-floor tools rather than symbols painted on wells. Each is a top-down ceramic-and-brass object with a simple silhouette, one readable material story, and empty atom seats left unobstructed. Fine valves, collars, channels, and service plates reward a close look; broad seats and channels carry meaning at board scale.

## How a machine texture is made

The game owns geometry and judgement; the generator only paints. `machines/gen.sh all` remakes every machine from `machines/manifest.toml`; `gen.sh NAME` remakes one; `gen.sh -k INDEX NAME` keeps a candidate by hand. A bible tweak is one edit to the manifest and one command.

1. **Scaffold.** `ziral --scaffold NAME` draws the machine's footprint from the one glyph table at 256 px per hex, turn 0: the cells as hexagonal wells sunk into plain clay, each seat marked by its role from that table (a blue-green ring for a seat that keeps its atom, a terracotta ring and dot for one that consumes it, a brass disc for the arm's pivot, an open terracotta horseshoe for its hand), inside a clay band that the sprite never shows. The pixel rectangle is the same `quad` the renderer places the sprite on, so alignment is by construction.
2. **Paint.** The image tool edits the scaffold in place, `candidates` times, with the manifest's shared prompt and the machine's own; the candidates stay under `NAME/candidates/`.
3. **Score.** `ziral --score` measures each candidate by code: `outside`, how far the clay outside the footprint drifted from clay; `seat`, the least contrast between any seat disc and the ring around it; `palette`, the distance from the sprite's mean colour to the nearest bible glaze. The thresholds are the manifest's; the best passing candidate is kept, its index recorded in the manifest, and `NAME/scores.tsv` keeps every number. The contact sheet `sheet.png` shows every candidate with its scores and the survivor.
4. **Cut.** `ziral --keep` cuts the kept candidate by the footprint mask into `NAME/albedo.png`, the flat base colour with baked ambient occlusion.

## Arm

A broad brass pivot, one-piece link, and terracotta rubber-lined horseshoe make a durable manipulator whose wear belongs only at joints and contact edges. Bushings, grease ports, and service plates imply maintenance without exposed clockwork.

## Source

A single blue-green ceramic feed hopper with one unmistakable seat, a brass metering ring, a short delivery chute, and a worn rubber gate. It reads as dispensing one bead, never storing a second.

## Bonder

Two adjacent cells as equal seats joined by a heavy compression bridge. Opposed clamps and shared pressure plumbing say that both atoms survive and leave joined; there is no third or sacrificial intake.

## Second bond

A triangular plum manifold with two matching bond seats and one different sacrificial feed. Paired rails connect the bond seats while the feed uses a funnel, one-way valve, and more severe collar, making the fixed roles legible before an atom arrives.

## Output

A two-cell ivory receiving station: two protected cups, a bond-checking bridge, paired rails, and one covered exit chute. It looks receptive rather than transformative.

## Cleanup

A one-cell soot-dark disposal well with a brass safety rim, rubber iris, covered ash chute, and quencher valve. It promises containment rather than machinery spectacle.
