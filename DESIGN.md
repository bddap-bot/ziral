# ziral — design plan

A hypothesis, not a spec. Short, living, rewritten after the first playable.

## Pitch

One sentence: who the player is, what they do, why that is fun.

Zoomed in, its a automation puzzle game, as the game progresses, as the player zooms out, it turns into an engineering game.

## Core loop

Zoomed loop: similar to Opus Magnum.
Wide loop: similar to Factorio.

The world is one continuous hex grid at every zoom level: every cell, pivot, and pad is a hex coordinate on it, and no machine has a boundary or a definition of its own; the wide view is nothing but zooming out. Each actuator gets a tape. A machine is only a primitive, an arm or a bonder or an applicator, the things later built as compounds and dropped on an output pad; there is no assembly-level machine, no group entity, and no group tape. Start with length 1 arms. We might copy opus magnum later or get creative. Single infinite source. Zoom is continuous; there is no boundary between micro and macro. Zoomed in, the puzzles are Opus Magnum-like; zooming out raises the level of abstraction. A group of machines can be copied and placed, as Factorio blueprints. There is no wrapping of a group into a new kind of entity. Mistakes at the micro level cause macro problems, and vice versa. The hard problems, as in Factorio, are planning, robustness, and managing complexity.

## Progression

Factorio's model. Progression unlocks mechanics and reveals the next challenge, each to be received with dismay. Science compounds are combined, then consumed to advance research. Each research tier demands new molecules, which demands new machines: that is what pulls the player back into the micro loop.

Each primitive machine is itself built as a compound and dropped on an output pad before it can be placed. Part of progression may fall out of the need to bootstrap, with no science at all. Open: how the first machine reaches the player's inventory. I guess science might not need to exist if we cleverly arrange dependencies. There is not first machine yet.

## Feel

Minute editing comes from Opus Magnum. Zoomed out editing comes from factorio, copy-paste included. Mouse and keyboard only.

## First playable

One micro editor: a small hex grid, two arms, bond and unbond, an instruction tape. One wide view: instances of that machine on a grid, joined by whatever transport the player builds from the same primitives. No readout. We might not ever need to provide an explicit goal. Graybox, circles and lines.

Status: Toy 1 (issue #1) exists to look at and play with while the design is imagined; the design is rewritten after it.

Falsifier: the machine gets designed once and never revisited. Then zoom-in is a tutorial, not a loop, and the pitch fails.

The bridge that should prevent that: Opus Magnum's three scores are Factorio's three pressures. Cost is resources, cycles is throughput, area is footprint. The wide game must demand a faster, smaller, or cheaper machine often enough that the player zooms back in.

## Out of scope

Art. Research tree. Enemies. Power. Fluids. Multiplayer. More than one molecule family. Save compatibility.

## Parking lot

- Creating a bond requires an atom: the atom becomes the bond between two other atoms.
- A jam element: a bane in the early game, until the player learns they need it and builds machines to manufacture it on purpose.
- Metals become transferable over long distances via a reaction resembling electroplating. Make it extra complicated, perhaps consuming a consumable on the receiving end.
- Select a machine by entering a code, like d-pad codes. The fun may be there.
- Clever matter-positive interactions between glyphs might be the progression later.
- A reification glyph taking an atom wrapped in two layers of fully bonded atoms of some specific type allows the player to add an atom to their inventory for manual placement.

## Open questions

How do we limit simulation load? Limit number of atoms?
Proposal: a finished machine is deterministic and periodic, so it compiles to a throughput function (period, inputs per period, outputs per period). The wide simulation runs the compiled form; the atom simulation runs only for machines in view or being edited. Cap atoms per machine, not per world. The compiled form is an internal optimization that must find periodic subgraphs itself; it is never a placement rule the player sees. Unresolved: what a machine does when an input belt is empty or an output belt is full (stall the whole tape, or per-arm waits) decides whether the compiled form stays exact.

How do we let player actively design and recover from mistakes. Debug step forward and back? Localized debug step?
Proposal: determinism gives step forward for free and step back by replay from a checkpoint. Localized step is the same on one machine with its recorded input stream.

Are placed instances linked to one definition (edit once, all update) or independent copies? Linked gives blueprints plus an upgrade path.

Are belts provided at all, or engineered from the primitives: grabbers moving a polymer, a corner meaning cut and re-bond after the turn, a favourite belt design copied? If engineered, two things follow. Copy-paste must make the fiftieth belt free, or transport becomes chores, so blueprints are core rather than a feature. And a hand-built belt costs far more to simulate than a provided one, so compiling blueprinted groups to a throughput function stops being an optimization and becomes the architecture. A third option: launchers. Single atoms can be launched; compounds need more involved transport. Transport cost then scales with what is moved, which is a decision in itself: move atoms and bond locally, or engineer compound transport.

Does the world run while you edit? Factorio's always-running world is fun: things go wrong while you think, and progress happens while you think. Opus Magnum would be unplayable in real time. Decided: entire world runs in lockstep.

What does a mistake look like in the world? Options: a local jam that persists until something clears it; no mistakes at all; the jam element from the parking lot; backpressure absorbing part of the problem. Decided: the world never halts; an illegal instruction stalls. On a tick, an instruction whose effect would be illegal (two arms moving into one cell, a grab of an atom another arm holds, a rotate that sweeps a held atom through an occupied cell) does not execute; that actuator's tape freezes on the instruction and retries every tick until it is legal, and everything else keeps running, so upstream backs up. Nothing is ever destroyed; deadlock is the failure mode and stays visible, with a marker on the stalled actuator. Conflicts between two actuators in the same tick resolve deterministically, in fixed actuator order.

Substrate: decided, vertical slice. One source atom type, two bond types, exercising the data model. What makes a compound valuable is still open.

What if arms could grab and move arms? Actuators as ordinary matter would make placement a machine act and give the bootstrap a path; not in the toy.

Bonding. Bonder glyph taking three atoms. One is destroyed, the other two become bonded. The sacrificial atom may in any of the three slots of the triangular glyph and must be of a specific atom type. If multiple atoms of that type are provided, the slots do have a priority to choose which atom is destroyed. The two bond types only differ in identity and must differ in visual appearance. Later recipes will require specific bond types. It's only counted as part of the vertical slice because it's kinda core to the data model. First bond type is single covalent. The same machine takes an additional sacrificial atom to convert the single bond to a double bond. It hurts discoverability to let the same machine do two things so early. The second-bond applicator is a different machine.

## Toy 1, first run (2026-09-04)

first run of ziral toy 1:
- selecting a machine give it focus, with focus asdf keys can be used for programming, use same shortcuts as opus magnum
- I am suprized glyphs can occupy the same hex, was that intentional? could be fun
- output glyphs will be similar to processing glyphs like the bonders (you may even be able to use the same abstraction/data-model for output,input, and processing.)
- where an atom goes on the glyph matters, glyph also cares about bond presence
- our single output glyph sucks up the entire compound despite it being two atoms double bonded together, any output glyphs should need to match the compounds shape in order to accept that compound. atom and bond identities must match too, though that may change for future output-style machines
- editing: quality of life: when holding a machine, we'll need to preview it visually
- when holding a machine keyboard keys should rotate it (copy opus magnum de re metalica controls)
- botom of screen should show all the tapes of machines currently on screen (up to a limit). click one tape to edit
- click and hold drags the machine, similarly click and drag from the bottom left array drags the machine, while dragging a preview is visible


oh, ziral needs a delete, lift a machine and hit a key to delete, same key as delete in opus magnum

in ziral, if a machine is dropped in an invalid location, it pops back to where it was picked up, if that's not possible it pops pack into users inventory. if we don't have inventory yet that just means it disappears

### Round 2, what Toy 1 now does

Machines never collide; only atoms do. Two glyphs on one hex was an accident of the first toy (placement never checked occupancy), kept as a rule: stacked glyphs each fire whenever their own rule matches, so a bonder under a second-bond glyph doubles a bond in two feeds.

Input, output, and processing glyphs are one model: a glyph is a list of slots, each an offset from its cell plus the atom type it wants, a list of every slot pair with the bond that must be present or absent there before it fires, the bonds it writes, and the slots it consumes. The source is a one-slot glyph that spawns when empty. The bonder and the second-bond applicator are three-slot glyphs that consume slot zero. The output is a two-slot glyph that fires only when the atoms on its slots are exactly one compound, with the double bond it asks for and no other bond; a compound of the right atoms turned the wrong way, or with anything else attached, sits on the glyph untouched.

Editing: a click focuses a machine; A and D turn it; Z deletes it. With an arm focused, F grab, R drop, E clockwise, Q counterclockwise, X wait write its tape at the cursor. Dragging a machine, or dragging from the palette, carries a preview under the pointer; A and D turn it, Z deletes it, and a release over the panels returns it to where it was lifted, or, from the palette, discards it. The strip along the bottom lists the tapes of the arms on screen, eight at most, and a click on one focuses that arm. Pan is right or middle drag.
