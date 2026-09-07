# Machine bible — Fired Workshop

The machines are compact kiln-floor tools standing on the board rather than symbols painted on wells or plates: a sprite carries only the machine, and the board's own tiles show around it. Each is a top-down ceramic-and-brass object with a simple silhouette, one readable material story, and empty atom seats left unobstructed. Fine valves, collars, channels, and service plates reward a close look; broad seats and channels carry meaning at board scale.

## How a machine texture is made

The game owns geometry and judgement; the generator only paints. `machines/gen.sh all` remakes every machine from `machines/manifest.toml`; `gen.sh NAME` remakes one; `gen.sh -k INDEX NAME` keeps a candidate by hand. A bible tweak is one edit to the manifest and one command.

1. **Scaffold.** `ziral --scaffold NAME` draws the machine's seat marks from the one glyph table at 256 px per hex, turn 0, on a flat pure green chroma key: each seat marked by its role from that table (a blue-green ring for a seat that keeps its atom, a terracotta ring and dot for one that consumes it, a brass disc for the arm's pivot, an open terracotta horseshoe for its hand), inside a green band that the sprite never shows. Nothing marks the footprint itself: any drawn cell, well or outline comes back as a plate under the machine, and a plate is a second set of tiles over the board's own. The pixel rectangle is the same `quad` the renderer places the sprite on, so alignment is by construction.
2. **Paint.** The image tool edits the scaffold in place, `candidates` times, with the manifest's shared prompt and the machine's own; the candidates stay under `NAME/candidates/`. `paint.sh` repaints a non-square return rather than forcing it to fit: the sprite goes on a square quad, so a forced fit would squash the art.
3. **Score.** `ziral --score` measures each candidate by code: `outside`, how much of the footprint's surround the machine covers, keyed by its green; `seat`, the least contrast between any seat disc and the ring around it; `palette`, the distance from the sprite's mean colour to the nearest bible glaze. The thresholds are the manifest's; the best passing candidate is kept, its index recorded in the manifest, and `NAME/scores.tsv` keeps every number. The contact sheet `sheet.png` shows every candidate with its scores and the survivor.
4. **Cut.** `ziral --keep` keys the kept candidate by its green into `NAME/albedo.png`, cropped to the quad, the flat base colour with baked ambient occlusion: every pixel the machine does not cover has alpha 0, so the board's tiles show through around it. The footprint mask judges a candidate; it never cuts one, so a machine ends at its own silhouette.
5. **Relief.** The kept candidate gets a matte grey sphere composited into the band (`NAME/relit/master.png`, remade by `--keep`, not kept in the repo) and is relit by the image tool once per edge the manifest names. `ziral --normals` reads each light's true direction and ambient share off the sphere, solves every pixel's normal by least squares, and writes `NAME/normal.png`; the sphere must come back as a sphere within the manifest's `sphere` degrees, else the four edits are painted again, three sets at most, before the run fails. `relit/lights.txt` records the recovered lights.

At runtime the sprite is a mesh with a tangent, drawn by `Lit`: albedo times a Lambert term under one fixed world light, with the normal rotated by the sprite's turn in the shader, so a rotated machine keeps its lit side on the world's light side and the arm's swing sweeps its highlight continuously.

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
