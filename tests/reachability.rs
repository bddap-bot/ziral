#![allow(dead_code)]

#[path = "../src/form.rs"]
mod form;
#[path = "../src/sim.rs"]
mod sim;

use form::{ATOM_ROUTES, AtomRoute, Form, RECIPES, atom_route};
use sim::{
    ArmLength, Atom, AtomKind, BondKind, DIRS, Glyph, GlyphKind, Hex, Instr, Item, Machine, ORIGIN,
    Sim, Spin, Tier,
};
const TOKENS: [Instr; 13] = [
    Instr::Grab,
    Instr::Drop,
    Instr::Rot(Spin::Ccw),
    Instr::Rot(Spin::Cw),
    Instr::Pivot(Spin::Ccw),
    Instr::Pivot(Spin::Cw),
    Instr::Wait,
    Instr::Move(0),
    Instr::Move(1),
    Instr::Move(2),
    Instr::Move(3),
    Instr::Move(4),
    Instr::Move(5),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fact {
    Item(Item),
    Compound(&'static str),
    Atom(AtomKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProofId {
    BaseAtom,
    Starting(Item),
    Compound(&'static str),
    RecipeItem(Item),
    ConverterAtom(AtomKind),
    ReifiedAtom(AtomKind),
    Broken,
    BrokenDependent,
    BrokenTransitive,
}

impl ProofId {
    fn fact(self) -> Fact {
        match self {
            ProofId::BaseAtom | ProofId::Broken => Fact::Atom(AtomKind::Base),
            ProofId::Starting(item) | ProofId::RecipeItem(item) => Fact::Item(item),
            ProofId::Compound(text) => Fact::Compound(text),
            ProofId::BrokenDependent => Fact::Compound(
                recipe_text(Item::Machine(Machine::Arm(ArmLength::One)))
                    .expect("the arm has a recipe"),
            ),
            ProofId::ConverterAtom(kind) => Fact::Atom(kind),
            ProofId::ReifiedAtom(kind) => Fact::Item(Item::Atom(kind)),
            ProofId::BrokenTransitive => Fact::Item(Item::Machine(Machine::Arm(ArmLength::One))),
        }
    }

    fn name(self) -> String {
        match self {
            ProofId::BaseAtom => atom_name(AtomKind::Base),
            ProofId::Starting(item) => root_item_name(item),
            ProofId::Compound(text) => compound_name(text),
            ProofId::RecipeItem(item) => recipe_item_name(item),
            ProofId::ConverterAtom(kind) => atom_name(kind),
            ProofId::ReifiedAtom(kind) => {
                format!("atom {kind:?} is attainable through reification")
            }
            ProofId::Broken => "broken atom Base is synthesizable".to_string(),
            ProofId::BrokenDependent => compound_name(
                recipe_text(Item::Machine(Machine::Arm(ArmLength::One)))
                    .expect("the arm has a recipe"),
            ),
            ProofId::BrokenTransitive => {
                recipe_item_name(Item::Machine(Machine::Arm(ArmLength::One)))
            }
        }
    }

    fn prove(self, premises: &[Fact]) -> Result<Fact, String> {
        match self {
            ProofId::BaseAtom => prove_base(premises),
            ProofId::Starting(item) => prove_start_item(item, premises),
            ProofId::Compound(text) => prove_compound(text, premises),
            ProofId::RecipeItem(item) => prove_item(item, premises),
            ProofId::ConverterAtom(kind) => prove_atom(kind, premises),
            ProofId::ReifiedAtom(kind) => prove_reified(kind, premises),
            ProofId::Broken => Err("deliberate break".to_string()),
            ProofId::BrokenDependent | ProofId::BrokenTransitive => {
                Err("a dependent proof ran".to_string())
            }
        }
    }

    fn premises(self) -> Result<Vec<ProofId>, String> {
        match self {
            ProofId::BaseAtom | ProofId::Starting(_) | ProofId::Broken => Ok(Vec::new()),
            ProofId::Compound(text) => compound_premises(text),
            ProofId::RecipeItem(item) => {
                let text = recipe_text(item)?;
                recipe_item_premises(item, text)
            }
            ProofId::ConverterAtom(kind) => {
                let AtomRoute::Converter(text) = atom_route(kind) else {
                    return Err("the base atom uses the source proof".to_string());
                };
                Ok(vec![
                    ProofId::RecipeItem(Item::Machine(Machine::Glyph(GlyphKind::Converter(kind)))),
                    ProofId::Compound(text),
                ])
            }
            ProofId::ReifiedAtom(kind) => reified_atom_premises(kind),
            ProofId::BrokenDependent => Ok(vec![ProofId::Broken]),
            ProofId::BrokenTransitive => Ok(vec![ProofId::BrokenDependent]),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Outcome {
    Passed(Fact),
    Failed(String),
    Skipped { premise: ProofId, reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum State {
    Pending,
    Visiting,
    Done(Outcome),
}

impl Outcome {
    fn reason(&self) -> Option<String> {
        match self {
            Outcome::Passed(_) => None,
            Outcome::Failed(reason) => Some(reason.clone()),
            Outcome::Skipped { premise, reason } => {
                Some(format!("premise {:?} failed: {reason}", premise.name()))
            }
        }
    }
}

struct Run {
    proofs: Vec<ProofId>,
    states: Vec<State>,
    calls: Vec<usize>,
}

impl Run {
    fn new(proofs: Vec<ProofId>) -> Self {
        let len = proofs.len();
        Run {
            proofs,
            states: vec![State::Pending; len],
            calls: vec![0; len],
        }
    }

    fn prove(&mut self, index: usize) -> Outcome {
        match &self.states[index] {
            State::Done(outcome) => return outcome.clone(),
            State::Visiting => {
                return Outcome::Failed("the dependency graph contains a cycle".to_string());
            }
            State::Pending => {}
        }
        self.states[index] = State::Visiting;
        let premise_ids = match self.proofs[index].premises() {
            Ok(premises) => premises,
            Err(reason) => {
                let outcome = Outcome::Failed(reason);
                self.calls[index] += 1;
                self.states[index] = State::Done(outcome.clone());
                return outcome;
            }
        };
        let mut premises = Vec::new();
        for id in premise_ids {
            let Some(premise_index) = self.proofs.iter().position(|proof| *proof == id) else {
                let outcome = Outcome::Failed(format!("premise {:?} has no proof", id.name()));
                self.states[index] = State::Done(outcome.clone());
                return outcome;
            };
            match self.prove(premise_index) {
                Outcome::Passed(fact) => premises.push(fact),
                outcome => {
                    let outcome = Outcome::Skipped {
                        premise: id,
                        reason: outcome.reason().unwrap(),
                    };
                    self.states[index] = State::Done(outcome.clone());
                    return outcome;
                }
            }
        }
        self.calls[index] += 1;
        let id = self.proofs[index];
        let outcome = match id.prove(&premises) {
            Ok(fact) if fact == id.fact() => Outcome::Passed(fact),
            Ok(fact) => Outcome::Failed(format!("returned {fact:?}, expected {:?}", id.fact())),
            Err(reason) => Outcome::Failed(reason),
        };
        self.states[index] = State::Done(outcome.clone());
        outcome
    }

    fn prove_all(&mut self) {
        for index in 0..self.proofs.len() {
            self.prove(index);
        }
    }

    fn report(&self) {
        for (proof, state) in self.proofs.iter().zip(&self.states) {
            let premises = match proof.premises() {
                Ok(ids) if ids.is_empty() => "none".to_string(),
                Ok(ids) => ids
                    .iter()
                    .map(|id| id.name())
                    .collect::<Vec<_>>()
                    .join(", "),
                Err(_) => "unresolved".to_string(),
            };
            let name = proof.name();
            let State::Done(outcome) = state else {
                panic!("proof {name} did not run")
            };
            match outcome {
                Outcome::Passed(_) => println!("PASS {name} <- {premises}"),
                Outcome::Failed(reason) => println!("FAIL {name} <- {premises}: {reason}"),
                Outcome::Skipped { premise, reason } => println!(
                    "SKIP {name} <- {premises}: premise {:?} failed: {reason}",
                    premise.name()
                ),
            }
        }
    }
}

fn atom_name(kind: AtomKind) -> String {
    format!("atom {kind:?} is synthesizable")
}

fn root_item_name(item: Item) -> String {
    format!("starting {item:?} is attainable")
}

fn compound_name(text: &'static str) -> String {
    if let Some((item, _)) = RECIPES.iter().find(|(_, recipe)| *recipe == text) {
        return format!("compound for {item:?} is synthesizable");
    }
    if let Some((kind, _)) = ATOM_ROUTES
        .iter()
        .find(|(_, route)| matches!(route, AtomRoute::Converter(input) if *input == text))
    {
        return format!("compound for the {kind:?} converter is synthesizable");
    }
    format!("compound {text:?} is synthesizable")
}

fn recipe_item_name(item: Item) -> String {
    if item == Item::Machine(Machine::Glyph(GlyphKind::SourceTwo)) {
        return "tier-two source is attainable by upgrading a placed source".to_string();
    }
    format!("{item:?} is attainable from its recipe")
}

fn require(premises: &[Fact], fact: Fact) -> Result<(), String> {
    if !premises.contains(&fact) {
        return Err(format!("{fact:?} was not granted"));
    }
    Ok(())
}

fn grant_item(sim: &mut Sim, premises: &[Fact], item: Item) -> Result<(), String> {
    require(premises, Fact::Item(item))?;
    sim.receive(item);
    Ok(())
}

fn grant_compound(
    sim: &mut Sim,
    premises: &[Fact],
    text: &'static str,
    turn: usize,
    at: Hex,
) -> Result<(), String> {
    require(premises, Fact::Compound(text))?;
    let form: Form = text
        .parse()
        .map_err(|reason| format!("{text:?}: {reason}"))?;
    let mut placed = form.sim();
    for atom in placed.atoms.iter_mut().flatten() {
        atom.pos = atom.pos.turned(turn).add(at);
    }
    sim.place(&placed, ORIGIN);
    Ok(())
}

fn grant_atom(sim: &mut Sim, premises: &[Fact], atom: Atom) -> Result<usize, String> {
    require(premises, Fact::Atom(atom.kind))?;
    Ok(sim.spawn(atom))
}

fn prove_base(premises: &[Fact]) -> Result<Fact, String> {
    if !premises.is_empty() {
        return Err("the base proof has a premise".to_string());
    }
    let source = sim::start()
        .glyphs
        .into_iter()
        .flatten()
        .find(|glyph| glyph.kind == GlyphKind::Source)
        .ok_or_else(|| "the starting world has no source".to_string())?;
    let mut sim = Sim::empty();
    sim.glyphs
        .push(Some(Glyph::new(source.kind, ORIGIN, source.dir)));
    sim.step();
    match sim.atom_at(ORIGIN).and_then(|id| sim.atoms[id]) {
        Some(Atom {
            kind: AtomKind::Base,
            ..
        }) => Ok(Fact::Atom(AtomKind::Base)),
        _ => Err("one source tick did not synthesize a base atom".to_string()),
    }
}

fn prove_start_item(item: Item, premises: &[Fact]) -> Result<Fact, String> {
    if !premises.is_empty() {
        return Err("a starting item proof has a premise".to_string());
    }
    let Item::Machine(Machine::Glyph(wanted)) = item else {
        return Err("the conclusion is not a starting glyph".to_string());
    };
    sim::start()
        .glyphs
        .iter()
        .flatten()
        .any(|glyph| glyph.kind == wanted)
        .then_some(Fact::Item(item))
        .ok_or_else(|| format!("the starting world has no {wanted:?}"))
}

fn take_glyph(sim: &mut Sim, kind: GlyphKind, at: Hex, dir: usize) -> Result<usize, String> {
    let item = Item::Machine(Machine::Glyph(kind));
    if !sim.inventory.spend(item) {
        return Err(format!("{item:?} was not granted"));
    }
    Ok(sim::seat(&mut sim.glyphs, Glyph::new(kind, at, dir)))
}

fn return_glyph(sim: &mut Sim, index: usize) -> Result<(), String> {
    let glyph = sim.glyphs[index]
        .take()
        .ok_or_else(|| "the placed glyph disappeared".to_string())?;
    sim.receive(Item::Machine(Machine::Glyph(glyph.kind)));
    Ok(())
}

fn direction(from: Hex, to: Hex) -> Result<usize, String> {
    let delta = to.sub(from);
    DIRS.iter()
        .position(|step| *step == delta)
        .ok_or_else(|| format!("{from:?} and {to:?} are not adjacent"))
}

fn write_single(sim: &mut Sim, a: usize, b: usize) -> Result<(), String> {
    let at = sim.atoms[a].ok_or_else(|| "the first bond atom is absent".to_string())?;
    let other = sim.atoms[b].ok_or_else(|| "the second bond atom is absent".to_string())?;
    let glyph = take_glyph(
        sim,
        GlyphKind::Bonder,
        at.pos,
        direction(at.pos, other.pos)?,
    )?;
    sim.step();
    let written = sim
        .bond_between(a, b)
        .filter(|index| sim.bonds[*index].kind == BondKind::Single)
        .is_some();
    return_glyph(sim, glyph)?;
    written
        .then_some(())
        .ok_or_else(|| format!("the bonder did not write {a}-{b}"))
}

fn write_second(sim: &mut Sim, premises: &[Fact], a: usize, b: usize) -> Result<(), String> {
    let a_pos = sim.atoms[a]
        .ok_or_else(|| "the first bond atom is absent".to_string())?
        .pos;
    let b_pos = sim.atoms[b]
        .ok_or_else(|| "the second bond atom is absent".to_string())?
        .pos;
    let placement = DIRS.iter().flat_map(|step| {
        let at = a_pos.add(*step);
        (0..DIRS.len()).map(move |dir| (at, dir))
    });
    let (at, dir) = placement
        .filter(|(at, _)| sim.atom_at(*at).is_none())
        .find(|(at, dir)| {
            let slots: Vec<Hex> = Glyph::new(GlyphKind::SecondBond, *at, *dir)
                .slots()
                .skip(1)
                .collect();
            slots == [a_pos, b_pos] || slots == [b_pos, a_pos]
        })
        .ok_or_else(|| format!("the bond {a}-{b} has no free second-bond placement"))?;
    let glyph = take_glyph(sim, GlyphKind::SecondBond, at, dir)?;
    let sacrifice = grant_atom(
        sim,
        premises,
        Atom {
            kind: AtomKind::Base,
            pos: at,
        },
    )?;
    sim.step();
    let written = sim
        .bond_between(a, b)
        .filter(|index| sim.bonds[*index].kind == BondKind::Double)
        .is_some();
    let consumed = sim.atoms[sacrifice].is_none();
    return_glyph(sim, glyph)?;
    (written && consumed)
        .then_some(())
        .ok_or_else(|| format!("the second-bond applicator did not upgrade {a}-{b}"))
}

fn prove_compound(text: &'static str, premises: &[Fact]) -> Result<Fact, String> {
    let form: Form = text
        .parse()
        .map_err(|reason| format!("{text:?}: {reason}"))?;
    let target = form.sim();
    let sim = construct(&target, premises)?;
    let made = Form::of(&sim);
    (made == form)
        .then_some(Fact::Compound(text))
        .ok_or_else(|| format!("constructed {made}, expected {form}"))
}

fn construct(target: &Sim, premises: &[Fact]) -> Result<Sim, String> {
    let mut sim = Sim::empty();
    for atom in target.atoms.iter().flatten() {
        grant_atom(&mut sim, premises, *atom)?;
    }
    if !target.bonds.is_empty() {
        grant_item(
            &mut sim,
            premises,
            Item::Machine(Machine::Glyph(GlyphKind::Bonder)),
        )?;
    }
    if target
        .bonds
        .iter()
        .any(|bond| bond.kind == BondKind::Double)
    {
        grant_item(
            &mut sim,
            premises,
            Item::Machine(Machine::Glyph(GlyphKind::SecondBond)),
        )?;
    }
    for bond in &target.bonds {
        write_single(&mut sim, bond.a, bond.b)?;
        if bond.kind == BondKind::Double {
            write_second(&mut sim, premises, bond.a, bond.b)?;
        }
    }
    Ok(sim)
}

fn prove_item(item: Item, premises: &[Fact]) -> Result<Fact, String> {
    let text = recipe_text(item)?;
    let form: Form = text
        .parse()
        .map_err(|reason| format!("{text:?}: {reason}"))?;
    if item == Item::Machine(Machine::Glyph(GlyphKind::SourceTwo)) {
        require(
            premises,
            Fact::Item(Item::Machine(Machine::Glyph(GlyphKind::Source))),
        )?;
        let mut compound = Sim::empty();
        grant_compound(&mut compound, premises, text, 0, ORIGIN)?;
        let mut sim = Sim::empty();
        sim.glyphs
            .push(Some(Glyph::new(GlyphKind::Source, ORIGIN, 0)));
        let events = sim
            .upgrade(0, &compound)
            .ok_or_else(|| "the placed source refused its upgrade compound".to_string())?;
        return (sim.glyphs[0].unwrap().kind == GlyphKind::SourceTwo
            && events.events.len() == 1
            && sim.inventory.count(item).is_none())
        .then_some(Fact::Item(item))
        .ok_or_else(|| "the upgrade did not produce a placed tier-two source".to_string());
    }
    let (tier, centre) = output_for(&form)
        .ok_or_else(|| format!("the recipe for {item:?} does not fit a shipped output"))?;
    let output = Item::Machine(Machine::Glyph(GlyphKind::Output(tier)));
    let mut sim = Sim::empty();
    grant_compound(&mut sim, premises, text, 0, ORIGIN)?;
    grant_item(&mut sim, premises, output)?;
    take_glyph(&mut sim, GlyphKind::Output(tier), centre, 0)?;
    sim.step();
    let count = sim.inventory.count(item);
    (count == Some(1) && sim.atoms.iter().all(Option::is_none))
        .then_some(Fact::Item(item))
        .ok_or_else(|| format!("the output left {item:?} at {count:?}"))
}

fn prove_atom(kind: AtomKind, premises: &[Fact]) -> Result<Fact, String> {
    let AtomRoute::Converter(text) = atom_route(kind) else {
        return Err("the base atom uses the source proof".to_string());
    };
    let converter = Item::Machine(Machine::Glyph(GlyphKind::Converter(kind)));
    let form: Form = text
        .parse()
        .map_err(|reason| format!("{text:?}: {reason}"))?;
    let glyph = Glyph::new(GlyphKind::Converter(kind), ORIGIN, 0);
    let (turn, at) = alignment(&form, glyph)?;
    let mut sim = Sim::empty();
    grant_compound(&mut sim, premises, text, turn, at)?;
    grant_item(&mut sim, premises, converter)?;
    take_glyph(&mut sim, GlyphKind::Converter(kind), ORIGIN, 0)?;
    sim.step();
    let atoms: Vec<Atom> = sim.atoms.iter().flatten().copied().collect();
    let pos = glyph
        .kind
        .product()
        .map(|offset| glyph.at.add(offset.turned(glyph.dir)))
        .ok_or_else(|| format!("the {kind:?} converter has no output"))?;
    (atoms == [Atom { kind, pos }])
        .then_some(Fact::Atom(kind))
        .ok_or_else(|| format!("the {kind:?} converter left {atoms:?}"))
}

fn prove_reified(kind: AtomKind, premises: &[Fact]) -> Result<Fact, String> {
    let machine = Machine::Glyph(GlyphKind::Reification);
    let (target, centre) = reification_target(kind)?;
    let mut sim = construct(&target, premises)?;
    grant_item(&mut sim, premises, Item::Machine(machine))?;
    take_glyph(&mut sim, GlyphKind::Reification, centre, 0)?;
    sim.step();
    let item = Item::Atom(kind);
    (sim.inventory.count(item) == Some(1) && sim.atoms.iter().all(Option::is_none))
        .then_some(Fact::Item(item))
        .ok_or_else(|| format!("reification did not make {item:?}"))
}

fn alignment(form: &Form, glyph: Glyph) -> Result<(usize, Hex), String> {
    let target: Vec<(Hex, Option<AtomKind>)> = glyph
        .slots()
        .zip(glyph.kind.rule().slots)
        .map(|(at, slot)| (at, slot.kind))
        .collect();
    let source = form.sim();
    for turn in 0..DIRS.len() {
        for atom in source.atoms.iter().flatten() {
            for (at, kind) in &target {
                if kind.is_some_and(|kind| kind != atom.kind) {
                    continue;
                }
                let offset = at.sub(atom.pos.turned(turn));
                let mut placed = source.clone();
                for atom in placed.atoms.iter_mut().flatten() {
                    atom.pos = atom.pos.turned(turn).add(offset);
                }
                let matches = placed.atoms.iter().flatten().all(|atom| {
                    target
                        .iter()
                        .any(|(at, kind)| *at == atom.pos && kind.is_none_or(|k| k == atom.kind))
                });
                if matches && placed.atoms.iter().flatten().count() == target.len() {
                    return Ok((turn, offset));
                }
            }
        }
    }
    Err(format!("{form} does not align with {:?}", glyph.kind))
}

fn output_for(form: &Form) -> Option<(Tier, Hex)> {
    [Tier::One, Tier::Two, Tier::Three]
        .into_iter()
        .find_map(|tier| form.centre(tier.radius()).map(|centre| (tier, centre)))
}

fn construction_premises(target: &Sim) -> Vec<ProofId> {
    let mut premises = Vec::new();
    if !target.bonds.is_empty() {
        premises.push(ProofId::Starting(Item::Machine(Machine::Glyph(
            GlyphKind::Bonder,
        ))));
    }
    let has_double = target
        .bonds
        .iter()
        .any(|bond| bond.kind == BondKind::Double);
    if has_double {
        premises.push(ProofId::RecipeItem(Item::Machine(Machine::Glyph(
            GlyphKind::SecondBond,
        ))));
    }
    let mut kinds: Vec<AtomKind> = target
        .atoms
        .iter()
        .flatten()
        .map(|atom| atom.kind)
        .collect();
    if has_double {
        kinds.push(AtomKind::Base);
    }
    kinds.sort_unstable();
    kinds.dedup();
    premises.extend(kinds.into_iter().map(|kind| {
        if kind == AtomKind::Base {
            ProofId::BaseAtom
        } else {
            ProofId::ConverterAtom(kind)
        }
    }));
    premises
}

fn compound_premises(text: &'static str) -> Result<Vec<ProofId>, String> {
    let form: Form = text
        .parse()
        .map_err(|reason| format!("{text:?}: {reason}"))?;
    Ok(construction_premises(&form.sim()))
}

fn recipe_text(item: Item) -> Result<&'static str, String> {
    RECIPES
        .iter()
        .find(|(made, _)| *made == item)
        .map(|(_, text)| *text)
        .ok_or_else(|| format!("{item:?} has no recipe"))
}

fn recipe_item_premises(item: Item, text: &'static str) -> Result<Vec<ProofId>, String> {
    if item == Item::Machine(Machine::Glyph(GlyphKind::SourceTwo)) {
        return Ok(vec![
            ProofId::Compound(text),
            ProofId::Starting(Item::Machine(Machine::Glyph(GlyphKind::Source))),
        ]);
    }
    let form: Form = text
        .parse()
        .map_err(|reason| format!("{text:?}: {reason}"))?;
    let (tier, _) = output_for(&form)
        .ok_or_else(|| format!("the recipe for {item:?} does not fit a shipped output"))?;
    let output = Item::Machine(Machine::Glyph(GlyphKind::Output(tier)));
    let output_premise = if tier == Tier::One {
        ProofId::Starting(output)
    } else {
        ProofId::RecipeItem(output)
    };
    Ok(vec![ProofId::Compound(text), output_premise])
}

fn reified_atom_premises(kind: AtomKind) -> Result<Vec<ProofId>, String> {
    let machine = Machine::Glyph(GlyphKind::Reification);
    let (target, _) = reification_target(kind)?;
    let mut premises = construction_premises(&target);
    premises.push(ProofId::RecipeItem(Item::Machine(machine)));
    Ok(premises)
}

fn reification_target(kind: AtomKind) -> Result<(Sim, Hex), String> {
    let machine = Machine::Glyph(GlyphKind::Reification);
    let text = recipe_text(Item::Machine(machine))?;
    let form: Form = text
        .parse()
        .map_err(|reason| format!("{text:?}: {reason}"))?;
    let centre = form
        .centre(Tier::Two.radius())
        .ok_or_else(|| "the reification input has no centre".to_string())?;
    let mut target = form.sim();
    let centre_atom = target
        .atom_at(centre)
        .ok_or_else(|| "the reification input has no centre atom".to_string())?;
    target.atoms[centre_atom].as_mut().unwrap().kind = kind;
    Ok((target, centre))
}

fn proofs() -> Vec<ProofId> {
    let output = Item::Machine(Machine::Glyph(GlyphKind::Output(Tier::One)));
    let bonder = Item::Machine(Machine::Glyph(GlyphKind::Bonder));
    let mut proofs = vec![
        ProofId::BaseAtom,
        ProofId::Starting(Item::Machine(Machine::Glyph(GlyphKind::Source))),
        ProofId::Starting(output),
        ProofId::Starting(bonder),
    ];
    let mut add = |proof| {
        if !proofs.contains(&proof) {
            proofs.push(proof);
        }
    };
    for (_, text) in RECIPES {
        add(ProofId::Compound(text));
    }
    for (_, route) in ATOM_ROUTES {
        let AtomRoute::Converter(text) = route else {
            continue;
        };
        add(ProofId::Compound(text));
    }
    for (item, _) in RECIPES {
        add(ProofId::RecipeItem(item));
    }
    for (kind, route) in ATOM_ROUTES {
        let AtomRoute::Converter(_) = route else {
            continue;
        };
        add(ProofId::ConverterAtom(kind));
    }
    for kind in AtomKind::ALL {
        add(ProofId::ReifiedAtom(kind));
    }
    proofs
}

#[test]
fn every_item_compound_and_atom_kind_is_reachable_in_world() {
    let mut run = Run::new(proofs());
    run.prove_all();
    run.report();
    let failures: Vec<String> = run
        .proofs
        .iter()
        .zip(&run.states)
        .filter_map(|(proof, state)| match state {
            State::Done(Outcome::Passed(_)) => None,
            other => Some(format!("{}: {other:?}", proof.name())),
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(run.calls.iter().all(|calls| *calls == 1));
    let calls = run.calls.clone();
    run.prove_all();
    assert_eq!(run.calls, calls);
    for (_, text) in RECIPES {
        assert!(run.proofs.contains(&ProofId::Compound(text)));
    }
    for machine in Machine::ALL {
        if machine == Machine::Glyph(GlyphKind::Source) {
            continue;
        }
        assert!(
            run.proofs
                .contains(&ProofId::RecipeItem(Item::Machine(machine)))
        );
    }
    for token in TOKENS {
        assert!(
            run.proofs
                .contains(&ProofId::RecipeItem(Item::Token(token)))
        );
    }
    for kind in AtomKind::ALL {
        let id = if kind == AtomKind::Base {
            ProofId::BaseAtom
        } else {
            ProofId::ConverterAtom(kind)
        };
        assert!(
            run.proofs
                .iter()
                .any(|proof| *proof == id && proof.fact() == Fact::Atom(kind))
        );
        assert!(run.proofs.iter().any(|proof| {
            *proof == ProofId::ReifiedAtom(kind) && proof.fact() == Fact::Item(Item::Atom(kind))
        }));
    }
}

#[test]
fn a_failed_premise_skips_every_dependent_and_names_it() {
    let broken = ProofId::Broken;
    let dependent = ProofId::BrokenDependent;
    assert_eq!(
        dependent.fact(),
        Fact::Compound(recipe_text(Item::Machine(Machine::Arm(ArmLength::One))).unwrap())
    );
    let mut run = Run::new(vec![broken, dependent, ProofId::BrokenTransitive]);
    run.prove_all();
    run.report();
    assert_eq!(run.calls, [1, 0, 0]);
    assert_eq!(
        run.states[1],
        State::Done(Outcome::Skipped {
            premise: broken,
            reason: "deliberate break".to_string(),
        })
    );
    assert!(matches!(
        &run.states[2],
        State::Done(Outcome::Skipped { premise, reason })
            if premise == &dependent && reason.contains("broken atom Base is synthesizable")
    ));
}
