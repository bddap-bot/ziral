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

A1: it is the player's job to put the molecule in the correct orientation.
A2: some processesors need release, outputs need release. Bond makers need release of the sacrificial atom but not of the atoms to be bonded (assuming the math works out)

### Round 2, what Toy 1 now does

Machines never collide; only atoms do. Two glyphs on one hex was an accident of the first toy (placement never checked occupancy), kept as a rule: stacked glyphs each fire whenever their own rule matches, so a bonder under a second-bond glyph doubles a bond in two feeds.

Input, output, and processing glyphs are one model: a glyph is a list of slots, each an offset from its cell plus the atom type it wants, a list of every slot pair with the bond that must be present or absent there before it fires, the bonds it writes, and the slots it consumes. The source is a one-slot glyph that spawns when empty. The bonder and the second-bond applicator are three-slot glyphs that consume slot zero. The output is a two-slot glyph that fires only when the atoms on its slots are exactly one compound, with the double bond it asks for and no other bond; a compound of the right atoms turned the wrong way, or with anything else attached, sits on the glyph untouched.

Editing: a click focuses a machine; A and D turn it; Z deletes it. With an arm focused, F grab, R drop, E clockwise, Q counterclockwise, X wait write its tape at the cursor. Dragging a machine, or dragging from the palette, carries a preview under the pointer; A and D turn it, Z deletes it, and a release over the panels returns it to where it was lifted, or, from the palette, discards it. The strip along the bottom lists the tapes of the arms on screen, eight at most, and a click on one focuses that arm. Pan is right or middle drag.

### Round 3, what Toy 1 now does

Round 2 asked two questions. Q1: an output's slots fix an orientation; is fitting the compound to the glyph's turn the player's job, or should an output take any of its shape's six turns? Q2: bonders fire on atoms an arm still holds or that sit inside a bigger molecule; only the output waits for a released, isolated compound; should processing glyphs also wait for release? The answers above decide both.

Decided: an output takes one orientation, its own. Turning the compound to fit is the player's job.

Decided: release is asked slot by slot. Each slot of a glyph says whether it demands that no hand be on the molecule its atom belongs to, and whether the atom is consumed. The source demands nothing. The bonder and the second-bond applicator demand release of the sacrificial slot only, so they fire on atoms an arm still holds or that sit inside a bigger molecule, and the arm that held a lone atom now holds the compound it became part of. The output demands it of every slot, and still asks for the exact shape and nothing attached. So far every consumed slot is also a released one.

The hand-held case balances: three atoms in, two atoms and a bond out, whatever hands are on the two survivors. If two arms hold the two atoms being bonded, both now hold one molecule and neither can rotate until one drops. If the sacrificial atom is itself bonded into a molecule nobody holds, it is consumed out of that molecule and its bonds go with it; a sacrificial atom bonded into a held molecule waits.

Questions for round 4:

Q1: A molecule can end up under two hands two ways: two arms each hold one of the atoms a bonder joins, or an arm grabs an atom inside a molecule another arm already holds. Every rotate on it then stalls until a tape drops. Is a molecule under two hands a deadlock the player programs around, as with any other stall, or should the bond and the grab that would put a second hand on it wait, as a glyph waits for release?

Q2: A bonder consumes a sacrificial atom out of the side of a molecule, severing its bonds, as long as no hand is on that molecule. Should the sacrificial slot also demand a lone atom, so consumption never breaks a bond the player made, or is eating an atom out of a molecule a tool?

### Round 4 answers

A1: stall is good in that situation
A2: let's let it tear the sacrificial atom off of compounds, could be fun

### Round 4, what Toy 1 now does

Decided: a molecule under two hands stalls every rotate until a tape drops it. The bond and the grab that put the second hand on it do not wait. The stall is shown: the stalled arm's pivot carries its white ring as before, and when the stall is another hand on the molecule, that hand carries a wider white ring, so the player can read which arm must drop. An arm records why it stalled, and only a stall caused by a hand names one.

Decided: a bonder or second-bond glyph tears its sacrificial atom out of an unheld compound. The atom's bonds are severed in the same tick the new bond is written, and what remains lies where it was, as one smaller compound or two. The tear is shown for that one tick: each severed bond stays drawn as a dim grey stub from its surviving atom toward the emptied slot. A sacrificial atom in a held compound still waits for release.

No code hedged either rule; nothing was deleted for them.

Questions for round 5:

Q1: A tear can split a compound. The toy eats an atom out of the middle of a chain and leaves both halves lying where they were, each now its own compound. Is a split a tool too, or should the sacrificial slot only take an atom at the end of a compound, with one bond to sever?

Q2: Within a tick, arms act before glyphs. A grab that lands on a compound the same tick a bonder would eat its sacrificial atom from it wins: the arm holds the compound and the bonder waits until the drop, then fires in the tick of the drop. Is arms-before-glyphs the order, or should glyphs read the state the tick began with?

### Round 5 answers

A1: Sacrificial slot destroys as many bonds as are connected to the sacrificial atom.
A2: maybe I don't fully follow. If an arm grabs the sacrificial atom in the same tick another arm drops it then it makes sense that it doesn't get dropped. Are the drop requirements for the bonders getting too complicated? We can simplify it would make the system more workable.

### Round 5, what Toy 1 now does

Decided: the sacrificial slot severs every bond on the sacrificial atom, wherever it sits. An atom eaten out of the middle of a chain leaves two compounds where one lay.

Round 4's Q2 in plainer terms. Each tick the source spawns, then the arms move one after another in a fixed order, then the glyphs fire. One arm is holding the sacrificial atom on a bonder and its tape says drop; another arm's tape says grab that same atom, same tick. If the dropper moves first: it drops, the other arm grabs, the bonder sees a held atom and does nothing. The atom is kept, as the answer expects. If the grabber moves first: the atom is still in the first arm's hand, so the grab stalls, and its ring names that hand. Then the first arm drops. The atom is now loose, and the bonder eats it. The grabber is left reaching at an empty cell forever.

The rule for when a bonder fires is simplified. Old rule:
- Each slot said two things: whether its atom is consumed, and whether it demands release.
- A slot demanding release waited while any hand was on any atom of the molecule its atom belongs to.
- The bonder and the second-bond glyph demanded it of the sacrificial slot; the output of both slots.
- So a sacrificial atom bonded into a held compound waited, though no hand was on it.

New rule:
- A glyph looks at its own slots at the end of the tick, and nothing else.
- It never eats an atom out of a hand: a held atom on a slot the glyph consumes means it does nothing this tick.
- Nothing else waits. A sacrificial atom bonded into a held compound is torn out, and the hand keeps the rest.

Consumed is the only thing a slot says now. Rounds 1 to 4 still hold: the atoms to be bonded may be held, an output takes only an exact compound with no hand on it since it consumes both atoms, a molecule under two hands still stalls its rotates. The one scene that changed is the old rule's last line. Also from this round: a grab stalled by another hand names that hand with the wider ring, as a rotate did since round 4.

Questions for round 6:

Q1: The simplified rule tears the sacrificial atom out of a compound an arm is holding, and the arm keeps what is left; before, that bonder waited for the drop. Is that the rule, or should a hand anywhere on the compound still keep its sacrificial atom from being eaten?

Q2: Your same-tick example comes out by arm order in the toy, dropper first keeping the atom and grabber first losing it. Is arm order a fine tiebreak here, or should a drop and a grab of one atom in one tick always pass it from hand to hand?

### Round 6 answers

A1: that's fine
A2: two grabbers can hold a compound at the same time, even by the same atom

Should we stop letting glyphs care whether an atom/compound is dropped or held? I could see a fun game either way so what's simpler?

### Round 6, what Toy 1 now does

Decided: a bonder tears its sacrificial atom out of a compound an arm holds, and the arm keeps what is left. Round 5's rule stands; nothing changed for it.

Decided: any number of hands may hold one atom, and so one compound. A grab never stalls because another hand is on the atom or its compound; the stall that named that hand is gone. Your same-tick example needs no tiebreak: the grabber's hand closes while the dropper's is still on, the drop takes one hand off, and the atom stays in the other. Arm order no longer decides it; both orders end in the same state. A rotate under two hands still stalls until a drop, as round 4 decided, and two grabs of one atom are now the shortest way to get there.

The simpler one, landed: glyphs do not care. Old rule:
- A glyph looked at its own slots at the end of the tick.
- A held atom on a slot the glyph consumes meant it did nothing this tick.
- So an output waited for the compound to be dropped, and a bonder waited while its sacrificial atom was in a hand.

New rule:
- A glyph looks at its own slots at the end of the tick and acts on whatever lies there, held or not.
- A hand holds whatever is at its cell. After a glyph eats, the hand is on what is left there: the rest of the compound, or nothing.
- No glyph waits on a hand.

What made it fall out: an arm no longer remembers which atom it holds. Its hand is open or closed, and a closed hand holds whatever is at its cell, so an eaten atom leaves the hand closed on an empty cell with no code to clear it. Deleted with that: the grab stall naming another hand, the glyph's look at hands, and what each hand remembered it held. Consequences the toy now shows: a closed hand on an empty cell keeps its ring, and an atom put there later is carried on the hand's next rotate; an output eats a compound straight out of a hand, and the arm is left closed on the pad. In your same-tick example with the bonder under it, the bonder eats the atom the tick the first hand grabs it, either order, and the second grab lands on an empty slot.

Questions for round 7:

Q1: A hand whose atom a glyph ate stays closed on the empty cell; the toy draws its ring on nothing, and an atom that another arm drops there is carried away by it on its next rotate. Should a closed hand catch whatever arrives under it, or open when its atom is gone?

Q2: An output now eats a compound an arm is holding, and the arm is left closed on the pad. That feeds an output with no drop and one instruction fewer. Is that a tool, or a trap for the player who parked a compound there meaning to come back for it?

### Round 7 answers

A1: oh a close hand should catch whatever lands under it, Unlike opus magnum
A2: that should be fine, players learn game mechanics

### Round 7, what Toy 1 now does

Decided: a closed hand catches whatever lands under it. Unlike Opus Magnum, an arm does not remember which atom it took; its hand is open or closed, and a closed hand holds whatever is at its cell. Round 6 built exactly that, so nothing changed and nothing needed deleting: no hand opens itself, no arm checks for the atom it grabbed, and the ring on a closed hand is the same ring on an atom or on nothing.

Decided: an output eats a compound an arm is holding, and the arm is left closed on the pad. Nothing changed.

What catching covers. Lands under it is not only a drop: an atom another arm carries through the cell under a closed hand is caught in passing, so both hands are on it, and the other arm's next rotate stalls, with the wider ring on the hand that caught it, until one of them drops. A closed hand on nothing that rotates onto an atom catches it the same way.

A grab over nothing does not make a closed hand on nothing. It stalls, retries every tick, and closes once an atom is under it, whether or not the other arm has dropped it. The program grab on the empty cell, wait, rotate does run, but its tape freezes on the grab until the atom arrives, and from then on runs late by as many ticks as it waited. A closed hand on nothing comes from a glyph eating its atom, or from turning or moving a closed arm in the editor.

Questions for round 8:

Q1: A closed hand catches an atom another arm carries through its cell, and that arm stalls until one of them drops; a closed hand rotated onto an atom catches it too. Does an atom carried through, or swept onto, count as landing under the hand, or should a closed hand catch only what is set down under it?

Q2: A grab over nothing stalls until an atom arrives, then closes on it, so the tape runs late by as many ticks as it waited. Since a closed hand now catches, should a grab over nothing just close the hand and let the tape run on, or is a grab with nothing to grab the mistake the stall says it is?

### Toy 1 shows motion between ticks

The sim is still one discrete tick after another; only the drawing changed. Between ticks the toy draws the way from the last pose to the next: an arm's rotate sweeps its 60° the way the instruction turns, the atoms in its hand ride the sweep, a grab shrinks the ring shut and a drop lets it grow and vanish, and what a glyph made or ate shows at the end of the sweep, so nothing changes hands mid-arc. One ease, smoothstep.

One knob: the tick period. An instruction's duration is its animation, as in Opus Magnum. Default 400 ms; the old 167 ms was too fast to read a sweep. Videos at 0 (hard cut), 120, 250, 400, 650 and 1000 ms, and 400 ms moving for the first 60% then holding, are for choosing it.
