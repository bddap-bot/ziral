# ziral — design plan

A hypothesis, not a spec. Short, living, rewritten after the first playable.

## Pitch

One sentence: who the player is, what they do, why that is fun.

Zoomed in, its a automation puzzle game, as the game progresses, as the player zooms out, it turns into an engineering game.

## Core loop

Zoomed loop: similar to Opus Magnum.
Wide loop: similar to Factorio.

Zoom is continuous; there is no boundary between micro and macro. Zoomed in, the puzzles are Opus Magnum-like; zooming out raises the level of abstraction. A group of machines can be copied and placed, as Factorio blueprints. There is no wrapping of a group into a new kind of entity. Mistakes at the micro level cause macro problems, and vice versa. The hard problems, as in Factorio, are planning, robustness, and managing complexity.

## Progression

Factorio's model. Progression unlocks mechanics and reveals the next challenge, each to be received with dismay. Science compounds are combined, then consumed to advance research. Each research tier demands new molecules, which demands new machines: that is what pulls the player back into the micro loop.

Each primitive machine is itself built as a compound and dropped on an output pad before it can be placed. Part of progression may fall out of the need to bootstrap, with no science at all. Open: how the first machine reaches the player's inventory. I guess science might not need to exist if we cleverly arrange dependencies. There is not first machine yet.

## Feel

Minute editing comes from Opus Magnum. Zoomed out editing comes from factorio, copy-paste included. Mouse and keyboard only.

## First playable

One micro editor: a small hex grid, two arms, bond and unbond, an instruction tape. One wide view: instances of that machine on a grid, joined by whatever transport the player builds from the same primitives. One goal: deliver N of one molecule per minute. Graybox, circles and lines.

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

## Open questions

How do we limit simulation load? Limit number of atoms?
Proposal: a finished machine is deterministic and periodic, so it compiles to a throughput function (period, inputs per period, outputs per period). The wide simulation runs the compiled form; the atom simulation runs only for machines in view or being edited. Cap atoms per machine, not per world. Unresolved: what a machine does when an input belt is empty or an output belt is full (stall the whole tape, or per-arm waits) decides whether the compiled form stays exact.

How do we let player actively design and recover from mistakes. Debug step forward and back? Localized debug step?
Proposal: determinism gives step forward for free and step back by replay from a checkpoint. Localized step is the same on one machine with its recorded input stream.

Are placed instances linked to one definition (edit once, all update) or independent copies? Linked gives blueprints plus an upgrade path.

Are belts provided at all, or engineered from the primitives: grabbers moving a polymer, a corner meaning cut and re-bond after the turn, a favourite belt design copied? If engineered, two things follow. Copy-paste must make the fiftieth belt free, or transport becomes chores, so blueprints are core rather than a feature. And a hand-built belt costs far more to simulate than a provided one, so compiling blueprinted groups to a throughput function stops being an optimization and becomes the architecture. A third option: launchers. Single atoms can be launched; compounds need more involved transport. Transport cost then scales with what is moved, which is a decision in itself: move atoms and bond locally, or engineer compound transport.

Does the world run while you edit? Factorio's always-running world is fun: things go wrong while you think, and progress happens while you think. Opus Magnum would be unplayable in real time. Decided: entire world runs in lockstep.

What does a mistake look like in the world? Options: a local jam that persists until something clears it; no mistakes at all; the jam element from the parking lot; backpressure absorbing part of the problem.

Substrate: decided, vertical slice. One source atom type, two bond types, exercising the data model. What makes a compound valuable is still open.
