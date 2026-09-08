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

One micro editor: a small hex grid, two arms, bond and unbond, an instruction tape. One wide view: instances of that machine on a grid, joined by whatever transport the player builds from the same primitives. No readout. We might not ever need to provide an explicit goal. Graybox, circles and lines, until the art bible (art/BIBLE.md) replaced it.

Status: Toy 1 (issue #1) exists to look at and play with while the design is imagined; the design is rewritten after it.

Falsifier: the machine gets designed once and never revisited. Then zoom-in is a tutorial, not a loop, and the pitch fails.

The bridge that should prevent that: Opus Magnum's three scores are Factorio's three pressures. Cost is resources, cycles is throughput, area is footprint. The wide game must demand a faster, smaller, or cheaper machine often enough that the player zooms back in.

## Out of scope

Research tree. Enemies. Power. Fluids. Multiplayer. More than one molecule family. Save compatibility.

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
Proposal: determinism gives step forward for free and step back by replay from a checkpoint. Localized step is the same on one machine with its recorded input stream. Decided for the world: step forward is one tick of the sim on a copy, step back is replay from the paused frame, no state log (Toy 1 steps through ghost frames). Localized step stays open.

Are placed instances linked to one definition (edit once, all update) or independent copies? Linked gives blueprints plus an upgrade path.

Are belts provided at all, or engineered from the primitives: grabbers moving a polymer, a corner meaning cut and re-bond after the turn, a favourite belt design copied? If engineered, two things follow. Copy-paste must make the fiftieth belt free, or transport becomes chores, so blueprints are core rather than a feature. And a hand-built belt costs far more to simulate than a provided one, so compiling blueprinted groups to a throughput function stops being an optimization and becomes the architecture. A third option: launchers. Single atoms can be launched; compounds need more involved transport. Transport cost then scales with what is moved, which is a decision in itself: move atoms and bond locally, or engineer compound transport.

Does the world run while you edit? Factorio's always-running world is fun: things go wrong while you think, and progress happens while you think. Opus Magnum would be unplayable in real time. Decided: entire world runs in lockstep.

What does a mistake look like in the world? Options: a local jam that persists until something clears it; no mistakes at all; the jam element from the parking lot; backpressure absorbing part of the problem. Decided: the world never halts; an illegal instruction stalls. On a tick, an instruction whose effect would be illegal does not execute: a grab over an empty cell; a rotate, pivot or move that would carry a held atom onto a cell holding an atom outside its compound, or onto any arm's base; a rotate, pivot or move of a molecule under two hands; a move whose base would land on a glyph, another arm's base or an atom. That actuator's tape freezes on the instruction and retries every tick until it is legal, and everything else keeps running, so upstream backs up. Nothing is ever destroyed; deadlock is the failure mode and stays visible, with a marker on the stalled actuator. Conflicts between two actuators in the same tick resolve deterministically, in fixed actuator order.

Substrate: decided, vertical slice. One source atom type, two bond types, exercising the data model. What makes a compound valuable is still open.

What if arms could grab and move arms? Actuators as ordinary matter would make placement a machine act and give the bootstrap a path; not in the toy.

Bonding. Decided. Two machines, one per bond type, each a glyph whose slots have fixed roles.

Directive (verbatim):

> design change for bondmaker glyphs. sacrificial atom is too much work for single bond. single bond glyph changes to be a two-tile glyph.
> the direction-agnosticism of the sacrificial atom make the rules complex without obvious benefit. change the double bonder to have dedicated slots for each input. This frees up the art department to paint a machine that more clearly indicates its usage.

The bonder is a two-slot glyph on two adjacent cells. When both slots hold an atom and no bond joins them, it writes a single bond and consumes nothing. The second-bond applicator is a three-slot glyph with fixed roles: slot zero is sacrificial and takes the atom type its table names; slots one and two must hold atoms that share a single bond. Slot zero takes a lone atom, one with no bond on it; a bonded atom there leaves the glyph unfired. It consumes slot zero and upgrades the bond to double. Only the bond between slots one and two is read: a single bond that instead joins the sacrificial slot to a bonded slot does not fire it. No slot search, no slot priority: the role is the position in the glyph's table, and the art paints each slot for its role. The two bond types differ only in identity and must differ in appearance; later recipes will require specific bond types.

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

Machines never collide; only atoms do. Two glyphs on one hex was an accident of the first toy (placement never checked occupancy), kept as a rule: stacked glyphs each fire whenever their own rule matches, so a bonder laid across a second-bond glyph's bonded slots makes a bond the second-bond glyph then doubles.

Input, output, and processing glyphs are one model: a glyph is a list of slots, each an offset from its cell plus the atom type it wants, a list of every slot pair with the bond that must be present or absent there before it fires, the bonds it writes, and the slots it consumes. The source is a one-slot glyph that spawns when empty. The bonder is a two-slot glyph that consumes nothing. The second-bond applicator is a three-slot glyph whose slot zero is sacrificial and consumed, and whose slots one and two must already share a single bond. The output is a two-slot glyph that fires only when the atoms on its slots are exactly one compound, with the double bond it asks for and no other bond; a compound of the right atoms turned the wrong way, or with anything else attached, sits on the glyph untouched.

Editing: a click on a machine focuses it; Z deletes it; A and D turn a glyph. An arm with focus acts now: F grab, R drop, A counterclockwise, D clockwise, Q pivot counterclockwise, E pivot clockwise, X wait run on the arm at once and write nothing, so A and D on a focused arm are the rotate instructions, hand and all, Q and E the pivots, with the stalls the tape would meet. The strip along the bottom lists the tapes of the arms on screen, eight at most, each instruction as its symbol token (art/SYMBOLS.md) with the running one outlined; a click on one focuses that tape and shows a cursor. With a tape focused the same keys insert at the cursor, left and right move it, home and end jump, Z is backspace, and escape or a click elsewhere leaves; nothing runs. Nothing on screen is written in any language: holding Tab holds up a page of the workshop's own manual (art/overlay/), the seven tokens woven into its flourishes, each beside a picture of what it does; letting go takes it away. Dragging a machine, or dragging from the palette, carries a preview under the pointer; A and D turn it, Z deletes it, and a release over the panels returns it to where it was lifted, or, from the palette, discards it. Pan is right or middle drag.

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

The sim is still one discrete tick after another; only the drawing changed. Between ticks the toy draws the way from the last pose to the next: an arm's rotate sweeps its 60° the way the instruction turns, the atoms in its hand ride the sweep, a grab shrinks the hand shut and a drop lets it open wide, and what a glyph made or ate shows at the end of the sweep, so nothing changes hands mid-arc. One ease for the sweep and the grip: the arm creeps a little as if pushing against a stop, the stop lets go, the arm runs to its target, overshoots, and rings out over the rest of the tick; a grab clenches past shut the same way. The reference sketch is art/reference/arm-swing-curve.jpg; art/reference/arm-swing-curve-fit.png lays the curve over it.

One knob: the tick period. An instruction's duration is its animation, as in Opus Magnum. Default 400 ms; the old 167 ms was too fast to read a sweep. Videos at 0 (hard cut), 120, 250, 400, 650 and 1000 ms, and 400 ms moving for the first 60% then holding, are for choosing it.

### Toy 1 wears the Fired Workshop look

The Fired Workshop direction (issue #9) carries two rules: every atom is visually distinct from every other atom, and the same for every glyph, machine, and bond type. `art/BIBLE.md` is the design ethos, palette, and the checkable form of the rules: two members of one class differ in at least two of hue, value, shape, marking, never by hue alone. The sim and the tick period are untouched; only the drawing changed.

What the toy now draws. The board is glazed clay with brass grout. An atom is a blue-green bead with one highlight. A single bond is a dark brass bar, a double bond two plum bars. An arm is a brass pivot and link with a terracotta horseshoe for its hand, open and wide at rest, shut on what it holds. Glyphs are wells sunk into the board: the source a blue-green ring and dot, the bonder a terracotta bar across two cells, the second bond a plum triangle with two spokes per slot and a ring on the sacrificial slot, the output two ivory cups with brass rims and the demanded bond stencilled between them. Ivory marks mean look here: the picked machine's cell, a stalled pivot, the hand that caused the stall.

The tests in `src/look.rs` hold the rule over the table the toy draws from, so a new atom, glyph, machine, or bond that collides with an old one does not land.

### Toy 1 wears diffusion textures

Directive (verbatim): "I want each machine, atom and bond in ziral to have a high res diffusion-generated texture. There should be many tile textures too. I'd like to see variance from tile to tile."

Every surface the toy draws now samples a 1024-square texture generated under the bible's style lock and kept in `art/textures/` with its prompt (issue #11): the arm's link and pivot, the bead, both bond bars, the well floor of every glyph kind, and twenty-four clay tiles. A cell's tile and its turn come from a hash of its coordinate, so the board varies from tile to tile and holds still under pan and zoom. The look table in `src/look.rs` names each texture beside its glaze, shape, and marking, and its tests now judge hue and value on the glaze and on the texture's mean colour together, require every row to wear its own texture, and require every tile to differ from every other at gameplay scale. Textures are compiled into the binary, so a missing one fails the build. Wells lost their flat fill; markings, rims, and ivory lines stay drawn over the textures.

### Toy 1 pivots

Directive (verbatim): "we'll still need atom pivot."

Pivot is the arm's second rotation, as in Opus Magnum: the held molecule turns 60° about the hand cell and the arm stays where it is. Q pivots counterclockwise, E clockwise. An open hand, or a closed hand on nothing, pivots nothing and the tape advances. With a held atom every atom of its component turns one step about the hand; the hand atom is the centre and stays put, and the arm's direction does not change. It stalls as a rotate stalls: a destination cell holding an atom outside the component freezes the tape on the instruction, and a molecule under two hands stalls with the wider ring on the other hand. Rotate and pivot are one motion, a rotation of the held component about a centre; rotate passes the arm's pivot and also turns the arm, pivot passes the hand. Between ticks the held atoms sweep their 60° about the hand under the one ease, the arm still. The tokens are Q and E in art/SYMBOLS.md.

### Toy 1 cleans up

Directive (verbatim): "add an atom cleanup machine. it consumes an atom then both the machine and the atom dissapear into the void. it breaks any bonds that were attached to the atom"

The cleanup glyph is a one-slot, single-use well: the tick an atom lies on it, every bond on that atom is severed, the atom is eaten, and the glyph is spent and leaves the board with it, the rest of the molecule lying where it was.

### Toy 1 draws in a fixed stack

Report (verbatim): "rather i see arms flickering over eachother per-frame"

No two machines ever share a depth. From the board up: glyph wells, torn bond stubs, bonds, arms, atoms with their rims, then whatever a drag is carrying, above all of it. Glyphs, arms and carried pieces each take a band, and a member's depth within its band follows its place in the list, later over earlier, the same fixed order the sim resolves actuator conflicts by, so two arms crossing one cell always show the same one on top. Atoms and bonds keep one depth per class: the sim allows one atom per cell and stalls a sweep into an occupied one, so they never overlap. Before this every arm shared one depth and the renderer broke the tie by entity id, which the drawing reassigns every frame; art/machines/proof/flicker.gif is two consecutive frames before and after, and the test in `src/main.rs` renders two frames of a still scene and requires them identical.

### Toy 1 grouts its tiles

Report (verbatim): "i see some solid lines between the hex tiles, would prefer to see only grout there"

Every tile hexagon was drawn at 0.95 of its cell with the whole square texture on it, so a seam read as three tones: the texture's painted grout stroke, the flat scaffold background beyond it, and the clear colour in the gap. The generated tiles also differ in how far their faces reach, 0.85 to 0.94 of the circumradius against the scaffold's 0.86, so no single crop fits them all. A tile now fills its cell and its material crops the texture to the face the tool painted plus one stroke of grout, the face measured from the texture's own pixels when the kiln fires (`look::face`); a seam is two strokes of grout meeting (issue #27). Cropping the PNGs once in the pipeline would draw the same seam, but the shipped art would no longer be the tool's output and a repainted tile would need the extra step; reading the face at fire time keeps every texture as painted. A headless render of an empty board asserts every pixel in the seam between two tiles lies within tolerance of the tiles' mean grout colour, every tile's crop must end in a band darker than two thirds of its face, and the stroke width is tied to `hex-scaffold.svg`. `art/textures/proof/grout.png` sets the seams before and after at the default zoom and at 4x.

### Toy 1 paints every seat on its cell's centre

Directive (verbatim): "the way our system works, textures will need to be designed to have their visual atom interface at the center of the tile. this is not the case for some of the machines in game right now"

A machine meets an atom at a seat, and the sim puts an atom at the centre of its cell, so every seat is painted at the exact centre of its cell, the arm's pivot and hand included; an off-centre seat shows the bead beside its cup. The pipeline registers every capture onto its cells by its seats and rejects what the fit cannot absorb; `art/MACHINES.md` carries the rule. `art/machines/proof/off-centre.png` sets every machine on its cells with each centre marked, before and after.

### Toy 1 grouts every tile from one template

Report (verbatim): "grout is looking nice. couple solid color seams here and there. maybe a common grout template to use as input to the tyle gneration would help"

Each tile carried its own grout. The tool painted every stroke with its own width, tone and softness, and the crop from issue #27 took whatever lay one stroke beyond the face it measured. At the shipped size the mip chain averages that band to one tone per tile, so a seam was two tones meeting, and where a band was flat the seam was a solid colour: tile-02 and tile-09 painted a thin stroke and the crop ran on into the flat scaffold background; tile-21 and tile-23 painted a flat dark stroke; tile-00, tile-06 and tile-16 painted a stroke so soft and wide that the crop clamped at the image edge and their seams read as clay. Over the 589 seams of the empty board the flattest had 0.008 grain and the farthest tone lay 0.20 from the grout mean, and the 0.15 tolerance of the #27 test admitted all of it.

Grout is now one image, `art/textures/grout.png`: a scaffold whose face is flat clay and whose surround, to the image edge, is coarse sanded grout generated once (issue #29). It is the input every tile is painted over, and when the kiln fires a tile it copies the template's pixels over everything beyond the scaffold stroke's inner edge (`look::grout`), so every tile's ring is the same pixels by construction and a seam is one grout meeting itself. The crop is the stroke's outer edge, a constant of `hex-scaffold.svg`; `look::face`, which measured each tile's reach, is gone, and no tile is judged on its border. Two shapes were disposed of. Grout as a board layer drawn once under tiles cropped to their faces needs a seamless grout texture tiled in world space, a second draw under the board, and still a crop every painted face must reach, which is the per-tile measurement back as a test. The template drawn as its own hexagon under each tile, the tile cropped to the ring, needs no composite, but the clay-to-grout edge becomes a polygon edge between two meshes that neither antialiases nor mips with the tile, and the board draws twice. Copying pixels keeps the edge inside one texture. The tile is still painted over the template so the tool's clay reaches the ring where the template's does and no stroke of its own lies inside it; a test holds every tile's clay bright out to the ring. The tool's own border in each PNG is never seen, so the shipped art stays the tool's output and a repainted tile needs no extra step.

The template is chosen for grain that survives the shipped minification: when the candidates were judged, a fine-grained first template rendered every seam at 0.010 grain, a smooth line, and the kept one rendered no seam under 0.022. The seam test samples every seam of the empty board and requires each within 0.05 of the template's band tone and above 0.015 grain; the template must be clay-faced and, on every edge and at every depth of its ring, darker than two thirds of its face with grain, since a tile never rotates and a seam sees one edge of it. All twenty-four tiles were painted again over the template. `art/textures/proof/grout-template.png` sets the seams before and after at the shipped size and the worst four at 4x.

### Toy 1 draws every machine from straight above

Directive (verbatim): "in order to make machines rotatable, the perspective of each should be direct-from top. we could alternatively generate six views for each machine but that may get tricky for arms rotating continuously"

The renderer already turns one sprite: a glyph's quad is rotated by its `dir` and the arm's by the angle from pivot to hand, and the arm's swing between ticks sweeps that angle continuously, so six pre-rendered views would have to be blended mid-sweep. What was missing was the drawing: the prompt asked for a straight-down view in passing, nothing measured it, and three of the six shipped machines came back from a raised camera. The source's hopper shows its back wall and its ring a far inner wall; the output's cups show their far walls with a thick near rim; the second-bond's three cups the same. Turned half a circle those walls swing to the near side and the machine reads as tipped over. The arm, bonder and cleanup are drawn from straight above and stay byte-identical.

The rule lives in `art/BIBLE.md` (View) and in the shared prompt (issue #30). A measurement was tried and disposed of: a concentric cup seen from straight above ought to be point-symmetric in tone about its centre, with a tilted camera lighting its far inner wall, and the six shipped machines did separate on it, flat ones at 0.08 to 0.12 against tipped ones at 0.16 to 0.22; but the candidates painted again under the new prompt, straight-down by eye, scored 0.16 to 0.24 in the same cups, because the tool shades a bowl's interior with a gradient whether or not the camera tilts. A score that cannot tell a straight-down view from a tipped one gives way to the eye, and the scaffold carries no new cue either: a plate or outline under the marks comes back as tiles (issue #25), and a straight-down view has no mark to draw. The source, output and second-bond were painted again and the kept one of each chosen by eye among the passing candidates; `art/machines/proof/top-down.png` sets every machine at the shipped size before and after, the changed ones at 4x.

### Toy 1 keeps every machine within its tiles

Directive (verbatim): "we are going to need to add a new rule for textures. machine textures must stay within the bounds of their n-tiles"

The sim puts a machine on n cells and nothing more; a sprite that paints past them covers a neighbour's tile, overlaps whatever stands there, and its edge lies about what the machine occupies. Three of the six shipped machines did: the bonder's kiln port and pipes sat in the notch between its two hexes, 0.37 of a circumradius beyond them; the cleanup's valve and hinge reached 0.23 past the vertices of its one hex; the source's rubber gate hung 0.20 below its hex. The old `outside` score, the mean keyed coverage of the footprint's surround inside the quad, admitted all three at its 0.15, since a port in a notch is a small share of a large surround (issue #33).

`outside` is now the farthest a pixel the key leaves visible lies beyond the union of the footprint's hexes, in circumradii, against the manifest's 0.05, two pixels at the shipped size: a bound is a maximum, not an average, so the mean is gone and the worst offender's distance is what the score reports. A test paints a dot half a tolerance and one and a half tolerances beyond a bonder's face and requires the first to pass and the second to fail, each reporting its own distance. The scaffold does not draw the footprint: a hairline in a darker shade of the key, chosen so the key would remove it if copied, came back from the tool as a clay hex plate under the machine in both probes, the failure issue #25 recorded for any drawn cell, well or outline, so the shared prompt states the rule in words and the score holds it. The bonder needed its own prompt to keep the port and pipes on the channel, since the notch between two adjacent hexes is where the tool put every raised fitting. The bonder, cleanup and source were painted again and the kept one of each chosen among the passing candidates by eye for the straight-down view; the arm, output and second-bond stay byte-identical. A compact bonder then read as alike to the output in the distinctness test, whose texture term averaged the difference over the whole quad on clay, so a sprite paid for being small; it now averages over the cells either sprite paints, which leaves every full-square texture as it was. `art/machines/proof/footprint.png` sets every machine at the shipped size before and after with its footprint drawn over it, the two plate probes beside them, the changed ones at 4x.

### Toy 1 caps a compound at 256 atoms

Directive (verbatim): "before i forget, let's put a limit on the size of a compound, bonders refuse if the compound created would be over a certain size. this is intended too reduce the risk of unbounded state size. does that make sense? 2^8 feels reasonable"

A compound has at most 256 atoms, `sim::MAX_COMPOUND_ATOMS`, the one constant. The bonder is the one glyph that writes a bond where none was, so it is the one place a compound grows; when the two compounds on its slots would together pass the cap it does not fire, and the atoms lie untouched as they do while a slot is empty: no marker, no text, nothing consumed. The check is in the glyph's match, keyed to the bonds a rule writes, so the second-bond applicator, whose rule demands the bond already there, never grows a compound and carries no check by construction. The bound is on per-compound state: a swing moves at most 256 atoms, an output's whole-shape test visits at most 256, and the check itself walks the two compounds it would join. Two alternatives disposed: a cap on the world's atom count adds nothing, since the sources' rate already bounds it; no cap leaves a runaway chain to grow without limit. 256 is far above anything built on purpose so far; only a runaway chain meets it. Whether a glyph refusing for many ticks is a stall is the deadlock question, decided elsewhere. A test bonds two chains that would make 257 atoms and requires every atom and bond unchanged, then two that make exactly 256 and requires the bond; a refusal at the 257th atom is not a picture worth making.

### Toy 1 takes a lone atom at the sacrificial slot

Directive (verbatim): "tear machinery gets removed. sacrificial atom must be single"

The second-bond applicator's sacrificial slot takes an atom with no bond on it. A bonded atom there leaves the glyph unfired, as an empty slot does: nothing consumed, no marker, the bond between slots one and two stays single. The demand is one more thing a slot says, beside the atom kind it wants and whether it is consumed, and the match reads it as it reads the kind; the cleanup glyph's slot says consumed alone, so it still eats a bonded atom and severs its bonds, as its round decided. This reverses rounds 4, 5 and 6 on the tear, which stay as history: a second bond severs no bond and splits no compound, and the one-tick tear is gone with the dim grey stubs that showed it, their depth band, and the half-alpha twin every texture was fired with for them. The hand rules stand: a lone sacrificial atom in a hand is eaten and the hand stays closed on its cell. Two alternatives disposed: keeping the tear as a tool makes the sacrificial intake a bond breaker too, a job the cleanup glyph already does; a slot that takes an end atom with one bond keeps the severing and the stub for one case and needs a bond count in the rule where the lone demand needs none. Tests: a sacrificial atom bonded to a fourth atom, with slots one and two single-bonded, leaves every atom and bond as it was; a lone one is eaten and the bond is double. Removing the demand fires the first; a refused fire is not a picture worth making.

### Toy 1 walks

Directive (verbatim): "arms get 6 new tape symbols, capital WEFCXZA move the arms base in the associated direction. W is up, C is down fill in the blanks from there."

Directive (verbatim): "Yes, arm base may not occupy the same tile as another arm base or a glyph. Or an atom."

Six instructions move the arm's base one cell, written with the shifted keys. The grid is pointy-top, a cell's vertices at 30°, 90° and on round, rows offset by half a cell, so no cell has an up neighbour and the ring follows the keyboard's own geometry around S and D: W is the upper-left neighbour, then clockwise E upper-right, F right, C lower-right, X lower-left, A left. Z is not used: W and C, E and X, F and A are the opposite pairs on that ring, Z's opposite R is not in the list, and six directions take six keys. The table is `KEYS` in `src/main.rs`, one row per token beside its glazes; the editor, the shot scenes and the tests read it, and the sim takes the row's direction alone, an index into `DIRS`: W 4, E 5, F 0, C 1, X 2, A 3.

A move translates the pivot and, with it, the hand and whatever the hand holds, by one cell; the arm's direction does not change. It is the third motion beside rotate and pivot and takes their path: the held compound is carried through one map of cells, a turn about the pivot, a turn about the hand, or a step of one cell, and the stall rules are the rules those two already had, plus one for the cell the base steps onto. A held compound whose atom would land on an atom outside it stalls (round 2). A molecule under two hands stalls a move as it stalls a rotate, with the wider ring on the other hand (round 4). A closed hand closes on whatever is at its new cell (rounds 6 and 7): a hand closed on nothing that moves onto an atom carries it from then on. A stalled move wears the ivory ring on its pivot as every stall does. The board has no edge, the world being one grid laid for whatever the camera shows, so a move never meets one. Between ticks the base, the arm and the held compound move their one cell under the same ease as a sweep. The pose at the end of a tape, which the ghost of #31 will draw, now includes position: for a tape that never stalls it is the fold of `Instr::posed` over the tape, the one pure function the sim executes through; a stalled tape ends short of it.

A base is a solid thing. An arm's base may not share a cell with another arm's base, a glyph, or an atom: a move whose base would land on any of them stalls as any illegal instruction does, and the rule is symmetric, so a rotate, pivot or move that would carry a held atom onto any arm's base, its own included, stalls too. This disposes of round 2's "Machines never collide; only atoms do" for arms: glyphs still stack on glyphs and an arm's hand still reaches over any glyph, but nothing stands on a base and a base stands on nothing. The editor still checks nothing when a machine is set down (round 2's accident, #52), so a base can still be placed on a glyph by hand; from its first move, rotate or pivot on, the rule holds.

The shifted keys write tokens only, inserted at the cursor of a focused tape; a focused arm runs nothing on them (#43 deletes immediate mode). Two shapes disposed: one move token with a direction written after it, which breaks one token per key and gives the tape a two-token instruction; and Z as a seventh key for a direction the hex has not got.

The six tokens are art/SYMBOLS.md: the letter hung from the arm's brass pivot pin as the rotates hang theirs, but slid one cell along a groove instead of swung, an arrowhead in the corner it slides toward, each opposite pair flipping figure and ground as A and D do, in the six glaze pairs the tape-size gate leaves once the seven are placed; four of them wear the board's clay, as letter or as tile, since a move is the arm crossing the board. Tests: the fold composes six moves to a ring and an opposite pair to a return, and leaves every other instruction's pose alone; a move carries the arm and its held compound; a held atom into an occupied cell stalls until it clears; a move under two hands names the other hand; a base onto a glyph, another base (earlier or later in arm order) or an atom stalls and onto a free cell moves; a pivot or rotate that would set a held atom on a base stalls; a closed hand moving onto an atom holds it; between ticks the base, hand and held atom slide as one; the shifted keys write to a focused tape and run nothing on a focused arm; the six keys ring the pivot clockwise from the upper left and sum to nothing; the `walk` scene repeats its lap with the same three-tick stall. `art/reference/proof/walk.gif` (`art/gif.sh`) is an arm walking a held atom round a closed path, stalled three ticks against a loose atom until a second arm swings it clear and back; `art/symbols/proof/moves-26px.png` is that scene's strip at tape size above a 4x enlargement, the six move tokens in a running tape with its stall mark, and `art/symbols/proof/tape-26px.png` is regenerated with all thirteen.

### Toy 1 steps through ghost frames

Directive (verbatim): "when the world is paused, it should be possible to step forward and back using two keys, (pick two that are easily accessible with left hand but not yet bound). player will be allowed to edit tape and step through time simultaneously so the step-forward/back buttons and other symbols writable to tape must not overlap. the canonical state of the world stays where it was when paused. on unpause the world resumes from its initial state. modifying a tape causes a re-sim from initial state to current ghost state. lets call the initial paused frame ghost[0]. machines can be modified at ghost[0] but not at ghost[>0], tapes *can* be modified at ghost[>0]."

Directive (verbatim): "the price is applied to the canonical state as if it were applied at ghost[0]. this is similar conceptually to tape edits, they apply to the canonical world too"

Directive (verbatim): "that rule seems like a fine way to state the policy. glyph editing after ghost[0] is fine if we resim. speakinig of, maybe it's cheaper to resim each step back rather than store a state log in memory?"

The model. ghost[0] is the canonical world at the moment of pause, and it is the only world the toy keeps; ghost[n] is exactly n ticks of the deterministic sim run from ghost[0] on a copy, with the tapes as they are now edited, so `Sim::replay(n)` is the one pure function every ghost comes from. G computes ghost[n+1] by one tick of the shown frame, which by determinism is `replay(n+1)`; S shows ghost[n−1] by replay from ghost[0], so there is no state log and nothing to keep in step with the world; a tape edit at any n is written into ghost[0] and the ghost is replayed to n. Unpause discards every ghost and the world resumes from ghost[0] as edited: the tapes as they were edited while stepping, and anything an edit or a step consumed (issue #38), are canonical, because every edit was applied to ghost[0] and never to a ghost. The open question's step-back-by-replay proposal is thereby decided.

The edit rule is by reason, not by category: an edit at ghost[n] is accepted iff its target exists unchanged at ghost[0], since only such a thing has a ghost[0] identity for the edit to apply to. A tape is never changed by the sim, so a tape edit is accepted at any n. A glyph never moves and is created by no tick, so placing, turning or deleting a glyph is accepted at any n, applied to ghost[0], then replayed; a glyph spent by a tick is gone from the ghost and cannot be picked there, but its id still names it at ghost[0]. An arm runs: its base, hand, grip and program counter at ghost[n] are the tape's doing, and what its hand is over at ghost[n] is the whole world's doing, so nothing about it at n is a thing that exists at ghost[0], a stalled arm included, whose hand sees a different cell at n; a place, move, turn, delete, copy, cut or immediate act on an arm at n>0 is refused, and the refusal is the toy's own: the drag pops back where it was lifted, as an invalid drop does, and a key does nothing. A fresh arm from the palette at n>0 is refused the same way, and a set dropped as one is accepted or refused as one. The toy has no atom drags, so there is nothing to refuse there. In code the rule is one predicate, `World::editable`: a glyph or a tape always, an arm only with no ghost shown.

Two alternatives disposed. An undo log, recording each tick's changes to walk back, is a second copy of the sim's semantics to keep correct and grows with n; replay costs n ticks of a sim whose tick is cheap and is correct by construction. Mutating the live world while stepping, with the paused frame stashed to restore on unpause, makes every editor read and write ambiguous between the two worlds, and an edit at n would land in a world that then has to be rewound; keeping ghost[0] as the one place edits go, and the ghost as a derived copy, needs no rewind at all.

Keys. Space pauses and unpauses, as before. S steps back, G steps forward, only while paused; both are home-row, left-hand, were bound to nothing, and no tape token is written with either, so a focused tape with its cursor open never sees them and stepping and editing interleave freely. G plays the sweep from ghost[n] into ghost[n+1] as a running tick does; S lands on ghost[n−1] settled, since the sim's between-tick motion is the instruction the earlier frame is about to run and has no reverse. The old `.` key, which paused and stepped forward in one press, is gone; S and G are the only step keys.

Drawing. ghost[n>0] is drawn in the bible's ghost language (art/BIBLE.md, Ghost): the frame alone, the canonical frame not under it, every machine, atom, bond and mark at ghost opacity with no relief and no light, over the board drawn as always. n is a row of ivory marks at the head of the tape strip, in fives, so the count is read as a tally and nothing is written. At ghost[0] paused the toy looks as it did.

Deleted. `Spent` and its index remap, and the focus remap that followed a spent glyph down the list: a spent or deleted glyph now leaves its slot empty, as an eaten atom does, and a placed one takes the first empty slot through the one `seat` the atoms already used, so nothing ever renumbers a glyph and an id names the same glyph in ghost[0] and in every ghost; the four `prev = sim.clone()` snapshots after edits, folded into the one replay; the `.` key; the world-stepping loop in the shot scenes, now a replay.


Tests: five G presses show `replay(5)` of an untouched ghost[0] with `replay(4)` behind it; a tape edit at three shows the ghost that stepping a fresh world with that tape three times shows, and equals `replay(3)`; an arm dragged, turned, acted on, deleted or placed at four is refused and ghost[0] and ghost[4] are byte-equal; a glyph placed at two is in ghost[0] and in the replayed ghost[2], and deleting it restores both; unpause at three restores ghost[0] exactly, drops every ghost, and S and G then do nothing; G and S on a focused tape with the cursor open leave the tape and the cursor as they were and move n; S at ghost[0] changes nothing. Mutation-tested both ways. `art/reference/proof/ghost.gif` (`art/gif.sh … ghost 15 …`) is the `ghost` shot scene: the walk paused, five steps forward through ghosts, a rotate written at the head of the walker's tape and the ghost replayed, three steps back, and unpause.

### Toy 1 deletes immediate mode

Directive (verbatim): "immediate-mode for developing muscle memory btw. im not sure it will ultimately fit, but its easy for a human to learn some controls with immediate results. ok to remove immediate mode for now if it wins simiplicity or elegance."

Deleted: the act path on a focused arm, by which F, R, A, D, Q and E ran on the arm at once and wrote nothing. It wins both. There is now one way for an arm to act, its tape; a person edits, through the pointer (#36) and the keys, and never acts. The immediate result the mode was for is still one keystroke away on a fresh arm: a token written into its empty tape and G (#35) shows that token performed, with the same keys, so the muscle memory it taught is kept; on a tape already running, the token runs when the head reaches it, which is what a tape is. One alternative disposed in a sentence: keeping immediate mode as a training toggle keeps a second way for an arm to act alive, with its own stall marker, its own refusal at ghost[n>0] and its own tests, to teach what write-then-step already teaches.

The focus rule as landed: a click on an arm focuses its tape, the cursor at the end, as a click on its row in the strip did; the keys insert there, left, right, home, end, Z, backspace and escape are the tape editor's, and a click elsewhere leaves; V pastes with nothing focused or a set picked, as before, so a focused tape takes no V. A drag still lifts the arm, since a focused tape names its arm as a pick does, and the drop leaves the set picked as every drop does; A and D turn a picked arm in place, Z deletes it, X cuts it, C copies it, so a single arm is turned or deleted by lifting it or by taking it in a marquee. A click on an arm inside a picked set keeps the set, so the set can be dragged, and its row in the strip focuses its tape. A click on a glyph picks it as before. The Tab manual page shows the tokens as before; nothing on it changes.

Deleted with the path: `World::act`, the one caller of `Sim::act` outside the tick; `focus_arm`, folded into `focus_tape`; the `armfocus` shot scene, while the scenes that selected an arm now focus its tape and show its cursor in the strip; the tests that acted on a focused arm, and the halves of the shifted-key and pivot-key tests that required a focused arm to run nothing, which a focused tape cannot do.

Tests: a click on an arm focuses its tape with the cursor at its end, and a second click after Home puts it there again; F then D on a clicked arm write grab and rotate, the hand stays open, the arm and the atom stay put, and no ghost is shown, then G grabs and a second G rotates with the atom carried; the press test reads the tape focus a press on an arm now gives. Mutation-tested both ways: a click that picks the arm instead, a cursor at the head, and a written token that also runs at once each fail their test for that reason. `art/reference/proof/write.gif` (`art/gif.sh … write 8 …`) is the `write` shot scene: paused, a click on an arm over an atom, F, then G, the hand shutting on the atom on the step.
