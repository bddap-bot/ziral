use crate::sim::{
    Atom, AtomKind, Bond, BondKind, DIRS, Glyph, GlyphKind, Hex, Instr, Item, MAX_COMPOUND_ATOMS,
    Machine, Sim, Spin, Tier,
};
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

pub const RECIPES: [(Item, &str); 22] = [
    (glyph(GlyphKind::Bonder), "B0,0 B0,1 0,0-0,1"),
    (
        glyph(GlyphKind::SecondBond),
        "B0,0 B0,1 B1,0 0,0-0,1 0,0-1,0 0,1-1,0",
    ),
    (Item::Machine(Machine::Arm), "B0,0 B0,1 0,0=0,1"),
    (
        glyph(GlyphKind::Converter(AtomKind::Amber)),
        "B0,0 B0,1 B1,0 B1,1 0,0-0,1 0,0=1,0 0,1-1,1",
    ),
    (
        glyph(GlyphKind::Output(Tier::One)),
        "B0,0 B0,1 B1,1 0,0-0,1 0,1-1,1",
    ),
    (
        glyph(GlyphKind::Output(Tier::Two)),
        "B0,1 B1,1 B1,2 B2,0 0,1-1,1 1,1-1,2 1,1-2,0",
    ),
    (
        glyph(GlyphKind::Converter(AtomKind::Plum)),
        "B0,0 B0,1 B0,2 A0,3 0,0-0,1 0,1=0,2 0,2-0,3",
    ),
    (
        glyph(GlyphKind::Output(Tier::Three)),
        "B0,1 B0,2 B1,0 B1,1 B1,2 B2,0 B2,1 0,1-1,1 0,2-1,1 1,0-1,1 1,1-1,2 1,1-2,0 1,1-2,1",
    ),
    (
        glyph(GlyphKind::Reification),
        "B0,2 B0,3 B0,4 B1,1 B1,2 B1,3 B1,4 B2,0 B2,1 B2,2 B2,3 B2,4 B3,0 B3,1 B3,2 B3,3 B4,0 B4,1 B4,2 0,2-0,3 0,2-1,1 0,2-1,2 0,3-0,4 0,3-1,2 0,3-1,3 0,4-1,3 0,4-1,4 1,1-1,2 1,1-2,0 1,1-2,1 1,2-1,3 1,2-2,1 1,2-2,2 1,3-1,4 1,3-2,2 1,3-2,3 1,4-2,3 1,4-2,4 2,0-2,1 2,0-3,0 2,1-2,2 2,1-3,0 2,1-3,1 2,2-2,3 2,2-3,1 2,2-3,2 2,3-2,4 2,3-3,2 2,3-3,3 2,4-3,3 3,0-3,1 3,0-4,0 3,1-3,2 3,1-4,0 3,1-4,1 3,2-3,3 3,2-4,1 3,2-4,2 3,3-4,2 4,0-4,1 4,1-4,2",
    ),
    (
        Item::Token(Instr::Grab),
        "B0,0 B0,1 B1,0 0,0-0,1 0,0-1,0 0,1=1,0",
    ),
    (Item::Token(Instr::Drop), "B0,0 B0,1 B0,2 0,0-0,1 0,1=0,2"),
    (
        Item::Token(Instr::Rot(Spin::Ccw)),
        "B0,0 B0,1 B1,1 0,0-0,1 0,1=1,1",
    ),
    (
        Item::Token(Instr::Rot(Spin::Cw)),
        "B0,0 B0,1 B1,1 0,0=0,1 0,1-1,1",
    ),
    (
        Item::Token(Instr::Pivot(Spin::Ccw)),
        "B0,0 B0,1 B1,0 0,0-0,1 0,1=1,0",
    ),
    (
        Item::Token(Instr::Pivot(Spin::Cw)),
        "B0,0 B0,1 B1,0 0,0-0,1 0,0=1,0",
    ),
    (Item::Token(Instr::Wait), "B0,0 B0,1 B0,2 0,0=0,1 0,1=0,2"),
    (
        Item::Token(Instr::Move(4)),
        "B0,0 B0,1 B0,2 B1,1 0,0=0,1 0,1-0,2 0,2-1,1",
    ),
    (
        Item::Token(Instr::Move(5)),
        "B0,0 B0,1 B1,1 B2,0 0,0=0,1 0,1-1,1 1,1-2,0",
    ),
    (
        Item::Token(Instr::Move(0)),
        "B0,0 B0,1 B1,0 B1,1 0,0-1,0 0,1-1,0 0,1=1,1",
    ),
    (
        Item::Token(Instr::Move(1)),
        "B0,0 B0,1 B0,2 B1,0 0,0-0,1 0,0-1,0 0,1=0,2",
    ),
    (
        Item::Token(Instr::Move(2)),
        "B0,0 B0,1 B1,1 B2,0 0,0-0,1 0,1-1,1 1,1=2,0",
    ),
    (
        Item::Token(Instr::Move(3)),
        "B0,0 B0,1 B1,0 B1,1 0,0-0,1 0,1-1,0 1,0=1,1",
    ),
];

#[derive(Clone, Copy)]
pub enum AtomRoute {
    Source,
    Converter(&'static str),
}

pub const ATOM_ROUTES: [(AtomKind, AtomRoute); 3] = [
    (AtomKind::Base, AtomRoute::Source),
    (
        AtomKind::Amber,
        AtomRoute::Converter("B0,0 B0,1 B1,0 0,0-0,1 0,0-1,0"),
    ),
    (
        AtomKind::Plum,
        AtomRoute::Converter("A0,0 B0,1 B1,1 0,0-0,1 0,1=1,1"),
    ),
];

pub fn atom_route(kind: AtomKind) -> AtomRoute {
    ATOM_ROUTES
        .iter()
        .find(|(made, _)| *made == kind)
        .map(|(_, route)| *route)
        .expect("an atom route")
}

pub fn atom_machine(kind: AtomKind) -> Machine {
    match atom_route(kind) {
        AtomRoute::Source => Machine::Glyph(GlyphKind::Source),
        AtomRoute::Converter(_) => Machine::Glyph(GlyphKind::Converter(kind)),
    }
}

const fn glyph(kind: GlyphKind) -> Item {
    Item::Machine(Machine::Glyph(kind))
}

pub fn recipes() -> &'static [(Item, Form)] {
    static FORMS: OnceLock<Vec<(Item, Form)>> = OnceLock::new();
    FORMS.get_or_init(|| {
        RECIPES
            .iter()
            .map(|(item, text)| {
                let form = text
                    .parse()
                    .unwrap_or_else(|e| panic!("the recipe of {item:?}: {e}"));
                (*item, form)
            })
            .collect()
    })
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Form {
    atoms: Vec<(Hex, AtomKind)>,
    bonds: Vec<(Hex, Hex, BondKind)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fragment(Sim);

impl Fragment {
    pub fn of(sim: &Sim) -> Fragment {
        let text = (0..6)
            .map(|turn| fragment_text(&posed(sim, turn)))
            .min()
            .expect("six turns");
        Fragment(parse_fragment(&text).expect("the writer makes a fragment"))
    }

    pub fn sim(&self) -> Sim {
        self.0.clone()
    }
}

impl Form {
    pub fn of(sim: &Sim) -> Form {
        let atoms: Vec<Atom> = sim.atoms.iter().flatten().copied().collect();
        (0..6)
            .map(|turn| {
                let at = |atom: &Atom| atom.pos.turned(turn);
                let origin = Hex::new(
                    atoms.iter().map(|a| at(a).q).min().unwrap_or(0),
                    atoms.iter().map(|a| at(a).r).min().unwrap_or(0),
                );
                let mut placed: Vec<(Hex, AtomKind)> = atoms
                    .iter()
                    .map(|atom| (at(atom).sub(origin), atom.kind))
                    .collect();
                let mut bonds: Vec<(Hex, Hex, BondKind)> = sim
                    .bonds
                    .iter()
                    .map(|bond| {
                        let end = |id: usize| at(&sim.atoms[id].unwrap()).sub(origin);
                        let (a, b) = (end(bond.a), end(bond.b));
                        (a.min(b), a.max(b), bond.kind)
                    })
                    .collect();
                placed.sort_unstable();
                bonds.sort_unstable();
                Form {
                    atoms: placed,
                    bonds,
                }
            })
            .min()
            .expect("six turns")
    }

    pub fn atoms(&self) -> &[(Hex, AtomKind)] {
        &self.atoms
    }

    pub fn centre(&self, radius: i32) -> Option<Hex> {
        let first = self.atoms.first()?.0;
        let within = |c: &Hex| self.atoms.iter().all(|(at, _)| at.sub(*c).ring() <= radius);
        self.atoms
            .iter()
            .map(|(at, _)| *at)
            .chain(DIRS.iter().map(|d| first.add(*d)))
            .find(within)
    }

    pub fn sim(&self) -> Sim {
        let mut sim = Sim::empty();
        for (pos, kind) in &self.atoms {
            sim.spawn(Atom {
                kind: *kind,
                pos: *pos,
            });
        }
        for (a, b, kind) in &self.bonds {
            let end = |at: Hex| sim.atom_at(at).expect("a bond joins atoms of the form");
            let bond = Bond {
                a: end(*a),
                b: end(*b),
                kind: *kind,
            };
            sim.bonds.push(bond);
        }
        sim
    }

    pub fn crafts(&self) -> Option<Item> {
        recipes()
            .iter()
            .find(|(_, form)| form == self)
            .map(|(item, _)| *item)
    }
}

fn letter(kind: AtomKind) -> char {
    match kind {
        AtomKind::Base => 'B',
        AtomKind::Amber => 'A',
        AtomKind::Plum => 'P',
    }
}

fn kind(letter: char) -> Option<AtomKind> {
    match letter {
        'B' => Some(AtomKind::Base),
        'A' => Some(AtomKind::Amber),
        'P' => Some(AtomKind::Plum),
        _ => None,
    }
}

fn cell(at: Hex) -> String {
    format!("{},{}", at.q, at.r)
}

impl fmt::Display for Form {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let atoms = self
            .atoms
            .iter()
            .map(|(at, kind)| format!("{}{}", letter(*kind), cell(*at)));
        let bonds = self.bonds.iter().map(|(a, b, kind)| {
            let mark = match kind {
                BondKind::Single => '-',
                BondKind::Double => '=',
            };
            format!("{}{mark}{}", cell(*a), cell(*b))
        });
        f.write_str(&atoms.chain(bonds).collect::<Vec<_>>().join(" "))
    }
}

fn machine_name(kind: GlyphKind) -> &'static str {
    match kind {
        GlyphKind::Source => "source",
        GlyphKind::Bonder => "bonder",
        GlyphKind::SecondBond => "second-bond",
        GlyphKind::Reification => "reification",
        GlyphKind::Converter(AtomKind::Base) => "base-converter",
        GlyphKind::Converter(AtomKind::Amber) => "amber-converter",
        GlyphKind::Converter(AtomKind::Plum) => "plum-converter",
        GlyphKind::Output(Tier::One) => "output-1",
        GlyphKind::Output(Tier::Two) => "output-2",
        GlyphKind::Output(Tier::Three) => "output-3",
    }
}

fn glyph_kind(name: &str) -> Option<GlyphKind> {
    match name {
        "bonder" => Some(GlyphKind::Bonder),
        "second-bond" => Some(GlyphKind::SecondBond),
        "reification" => Some(GlyphKind::Reification),
        "amber-converter" => Some(GlyphKind::Converter(AtomKind::Amber)),
        "plum-converter" => Some(GlyphKind::Converter(AtomKind::Plum)),
        "output-1" => Some(GlyphKind::Output(Tier::One)),
        "output-2" => Some(GlyphKind::Output(Tier::Two)),
        "output-3" => Some(GlyphKind::Output(Tier::Three)),
        _ => None,
    }
}

fn instruction_letter(instr: Instr) -> char {
    match instr {
        Instr::Grab => 'F',
        Instr::Drop => 'R',
        Instr::Rot(Spin::Ccw) => 'A',
        Instr::Rot(Spin::Cw) => 'D',
        Instr::Pivot(Spin::Ccw) => 'Q',
        Instr::Pivot(Spin::Cw) => 'E',
        Instr::Wait => 'X',
        Instr::Move(4) => 'w',
        Instr::Move(5) => 'e',
        Instr::Move(0) => 'f',
        Instr::Move(1) => 'c',
        Instr::Move(2) => 'x',
        Instr::Move(3) => 'a',
        Instr::Move(_) => unreachable!("six move directions"),
    }
}

fn instruction(letter: char) -> Option<Instr> {
    match letter {
        'F' => Some(Instr::Grab),
        'R' => Some(Instr::Drop),
        'A' => Some(Instr::Rot(Spin::Ccw)),
        'D' => Some(Instr::Rot(Spin::Cw)),
        'Q' => Some(Instr::Pivot(Spin::Ccw)),
        'E' => Some(Instr::Pivot(Spin::Cw)),
        'X' => Some(Instr::Wait),
        'w' => Some(Instr::Move(4)),
        'e' => Some(Instr::Move(5)),
        'f' => Some(Instr::Move(0)),
        'c' => Some(Instr::Move(1)),
        'x' => Some(Instr::Move(2)),
        'a' => Some(Instr::Move(3)),
        _ => None,
    }
}

fn fragment_text(sim: &Sim) -> String {
    let mut lines: Vec<String> = sim
        .glyphs
        .iter()
        .flatten()
        .map(|g| format!("{} {} {}", machine_name(g.kind), cell(g.at), g.dir))
        .chain(sim.arms.iter().map(|a| {
            let tape = if a.tape.is_empty() {
                "-".to_string()
            } else {
                a.tape.iter().copied().map(instruction_letter).collect()
            };
            format!("arm {} {} {tape} {}", cell(a.pivot), a.dir, a.pc)
        }))
        .collect();
    let mut seen = Vec::new();
    for i in sim
        .atoms
        .iter()
        .enumerate()
        .filter_map(|(i, a)| a.map(|_| i))
    {
        if seen.contains(&i) {
            continue;
        }
        let compound = sim.component(i);
        seen.extend(&compound);
        let form = sim.fragment(&compound, Hex::new(0, 0));
        lines.push(compound_text(&form));
    }
    lines.sort_unstable();
    lines.join("\n")
}

fn compound_text(sim: &Sim) -> String {
    let mut atoms: Vec<(Hex, AtomKind)> = sim
        .atoms
        .iter()
        .flatten()
        .map(|a| (a.pos, a.kind))
        .collect();
    let mut bonds: Vec<(Hex, Hex, BondKind)> = sim
        .bonds
        .iter()
        .map(|bond| {
            let (a, b) = (
                sim.atoms[bond.a].unwrap().pos,
                sim.atoms[bond.b].unwrap().pos,
            );
            (a.min(b), a.max(b), bond.kind)
        })
        .collect();
    atoms.sort_unstable();
    bonds.sort_unstable();
    Form { atoms, bonds }.to_string()
}

fn posed(sim: &Sim, turn: usize) -> Sim {
    let ids: Vec<usize> = sim
        .atoms
        .iter()
        .enumerate()
        .filter_map(|(i, atom)| atom.map(|_| i))
        .collect();
    let mut posed = sim.fragment(&ids, Hex::new(0, 0));
    posed.arms = sim
        .arms
        .iter()
        .map(|a| {
            let mut arm =
                crate::sim::Arm::new(a.pivot.turned(turn), (a.dir + turn) % 6, a.tape.clone());
            arm.pc = a.pc;
            arm
        })
        .collect();
    posed.glyphs = sim
        .glyphs
        .iter()
        .flatten()
        .map(|g| Some(Glyph::new(g.kind, g.at.turned(turn), (g.dir + turn) % 6)))
        .collect();
    for atom in posed.atoms.iter_mut().flatten() {
        atom.pos = atom.pos.turned(turn);
    }
    let positions = posed
        .arms
        .iter()
        .map(|a| a.pivot)
        .chain(posed.glyphs.iter().flatten().map(|g| g.at))
        .chain(posed.atoms.iter().flatten().map(|a| a.pos));
    let positions: Vec<Hex> = positions.collect();
    let origin = Hex::new(
        positions.iter().map(|at| at.q).min().unwrap_or(0),
        positions.iter().map(|at| at.r).min().unwrap_or(0),
    );
    for arm in &mut posed.arms {
        arm.pivot = arm.pivot.sub(origin);
    }
    for glyph in posed.glyphs.iter_mut().flatten() {
        glyph.at = glyph.at.sub(origin);
    }
    for atom in posed.atoms.iter_mut().flatten() {
        atom.pos = atom.pos.sub(origin);
    }
    posed
}

impl fmt::Display for Fragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&fragment_text(&self.0))
    }
}

fn parse_cell(text: &str) -> Result<Hex, String> {
    let bad = || format!("{text:?} is not a cell");
    let (q, r) = text.split_once(',').ok_or_else(bad)?;
    let coordinate = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse::<i32>().ok())
            .flatten()
            .ok_or_else(bad)
    };
    Ok(Hex::new(coordinate(q)?, coordinate(r)?))
}

fn parse_compound(text: &str) -> Result<Sim, String> {
    let mut sim = Sim::empty();
    let (atoms, bonds): (Vec<&str>, Vec<&str>) = text
        .split_whitespace()
        .partition(|token| token.starts_with(|c: char| c.is_ascii_alphabetic()));
    for token in atoms {
        let kind = kind(token.chars().next().unwrap())
            .ok_or_else(|| format!("{token:?} is no atom kind"))?;
        let pos = parse_cell(&token[1..])?;
        if sim.atom_at(pos).is_some() {
            return Err(format!("two atoms at {token:?}"));
        }
        sim.spawn(Atom { kind, pos });
    }
    for token in bonds {
        let k = token
            .find(['-', '='])
            .ok_or_else(|| format!("{token:?} is neither an atom nor a bond"))?;
        let kind = match &token[k..=k] {
            "-" => BondKind::Single,
            _ => BondKind::Double,
        };
        let end = |s: &str| {
            sim.atom_at(parse_cell(s)?)
                .ok_or_else(|| format!("{token:?} bonds a cell with no atom"))
        };
        let (a, b) = (end(&token[..k])?, end(&token[k + 1..])?);
        if a == b {
            return Err(format!("{token:?} bonds a cell to itself"));
        }
        if sim.bond_between(a, b).is_some() {
            return Err(format!("{token:?} bonds twice"));
        }
        sim.bonds.push(Bond { a, b, kind });
    }
    let count = sim.atoms.len();
    if count == 0 {
        return Err("no atom".to_string());
    }
    if count > MAX_COMPOUND_ATOMS {
        return Err(format!("{count} atoms, over {MAX_COMPOUND_ATOMS}"));
    }
    if sim.component(0).len() != count {
        return Err("not one compound".to_string());
    }
    Ok(sim)
}

fn number(text: &str, thing: &str) -> Result<usize, String> {
    (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse::<usize>().ok())
        .flatten()
        .ok_or_else(|| format!("{text:?} is not a {thing}"))
}

fn parse_machine(line: &str, sim: &mut Sim) -> Result<(), String> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    let name = fields[0];
    let mut machine = Sim::empty();
    if name == "arm" {
        if fields.len() != 5 {
            return Err(format!("{line:?} is not an arm line"));
        }
        let pivot = parse_cell(fields[1])?;
        let dir = number(fields[2], "turn")?;
        if dir >= 6 {
            return Err(format!("{} is not one of six turns", fields[2]));
        }
        let tape = if fields[3] == "-" {
            Vec::new()
        } else {
            fields[3]
                .chars()
                .map(|letter| {
                    instruction(letter)
                        .ok_or_else(|| format!("{letter:?} is not an instruction letter"))
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut arm = crate::sim::Arm::new(pivot, dir, tape);
        arm.pc = number(fields[4], "program counter")?;
        machine.arms.push(arm);
    } else {
        if fields.len() != 3 {
            return Err(format!("{line:?} is not a machine line"));
        }
        let kind = glyph_kind(name).ok_or_else(|| format!("{name:?} is no machine kind"))?;
        let at = parse_cell(fields[1])?;
        let dir = number(fields[2], "turn")?;
        if dir >= 6 {
            return Err(format!("{} is not one of six turns", fields[2]));
        }
        machine.glyphs.push(Some(Glyph::new(kind, at, dir)));
    }
    if !sim.fits(&machine, Hex::new(0, 0), &[]) {
        return Err(format!("{line:?} overlaps the fragment"));
    }
    sim.place(&machine, Hex::new(0, 0));
    Ok(())
}

fn parse_fragment(text: &str) -> Result<Sim, String> {
    let mut sim = Sim::empty();
    let mut any = false;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        any = true;
        let first = line.split_whitespace().next().unwrap();
        if first == "arm" || glyph_kind(first).is_some() || first == "source" {
            parse_machine(line, &mut sim)?;
            continue;
        }
        let compound = parse_compound(line)?;
        if !sim.fits(&compound, Hex::new(0, 0), &[]) {
            return Err(format!("{line:?} overlaps the fragment"));
        }
        sim.place(&compound, Hex::new(0, 0));
    }
    if !any {
        return Err("empty fragment".to_string());
    }
    Ok(sim)
}

impl FromStr for Form {
    type Err = String;

    fn from_str(text: &str) -> Result<Form, String> {
        Ok(Form::of(&parse_compound(text)?))
    }
}

impl FromStr for Fragment {
    type Err = String;

    fn from_str(text: &str) -> Result<Fragment, String> {
        Ok(Fragment::of(&parse_fragment(text)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spokes(centre: Hex, turn: usize) -> Sim {
        let mut sim = Sim::empty();
        let hub = sim.spawn(Atom {
            kind: AtomKind::Base,
            pos: centre,
        });
        for k in [0, 2, 4] {
            let spoke = sim.spawn(Atom {
                kind: AtomKind::Base,
                pos: centre.add(DIRS[k].turned(turn)),
            });
            let tip = sim.spawn(Atom {
                kind: AtomKind::Base,
                pos: centre.add(DIRS[k].add(DIRS[(k + 1) % 6]).turned(turn)),
            });
            sim.bonds.push(Bond {
                a: hub,
                b: spoke,
                kind: BondKind::Single,
            });
            sim.bonds.push(Bond {
                a: spoke,
                b: tip,
                kind: BondKind::Double,
            });
        }
        sim
    }

    #[test]
    fn writing_then_parsing_every_recipe_is_identity_and_the_table_is_written_as_it_is_written() {
        for ((item, text), (_, form)) in RECIPES.iter().zip(recipes()) {
            let written = form.to_string();
            assert_eq!(written.parse::<Form>().as_ref(), Ok(form), "{item:?}");
            assert_eq!(
                &written, text,
                "{item:?} is not in canonical form in the table"
            );
            assert_eq!(Form::of(&form.sim()), *form, "{item:?}");
            assert_eq!(form.crafts(), Some(*item));
        }
        let arm = Machine::Arm.recipe().unwrap();
        assert_eq!(arm.sim().bonds[0].kind, BondKind::Double);
        assert_eq!(arm.to_string(), "B0,0 B0,1 0,0=0,1");
        assert_eq!(
            "0,1-0,0 B0,1 B0,0".parse::<Form>().unwrap(),
            *Machine::Glyph(GlyphKind::Bonder).recipe().unwrap()
        );
        assert_eq!(
            "B7,3 B8,2 7,3=8,2\n".parse::<Form>().unwrap().to_string(),
            arm.to_string()
        );
        for bad in [
            "",
            "B0,-1",
            "B0,+1",
            "B-0,1",
            "B0,0 B0,1 0,0--0,1",
            "B0,0 0,0-0,1",
            "B0,0 B0,1 0,0-0,1 0,1-0,0",
            "B0,0 B0,0",
            "B0,0 B0,1 x",
            "B0,0 B0,1 Q0,2",
            "B0,0 0,0-0,0",
            "B0,0 B0,1",
            "B0,0 B0,1 0,0-0,1 B5,5 B5,6 5,5-5,6",
        ] {
            assert!(bad.parse::<Form>().is_err(), "{bad:?} parsed");
        }
        let chain: String = (0..=MAX_COMPOUND_ATOMS as i32)
            .map(|r| format!("B0,{r}"))
            .chain((0..MAX_COMPOUND_ATOMS as i32).map(|r| format!("0,{r}-0,{}", r + 1)))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            chain.parse::<Form>(),
            Err("257 atoms, over 256".to_string())
        );
    }

    #[test]
    fn the_same_compound_placed_twice_moved_and_turned_writes_identical_text() {
        let text = Form::of(&spokes(Hex::new(0, 0), 0)).to_string();
        for turn in 0..6 {
            let moved = spokes(Hex::new(7, -3), turn);
            assert_eq!(Form::of(&moved).to_string(), text, "turn {turn}");
            assert_eq!(text.parse::<Form>().unwrap(), Form::of(&moved));
        }
        assert_eq!(
            text,
            "B0,0 B0,2 B0,3 B1,0 B1,1 B2,1 B3,0 0,0=1,0 0,2=0,3 0,2-1,1 1,0-1,1 1,1-2,1 2,1=3,0"
        );
        assert_eq!(text.len(), 82);
    }

    #[test]
    fn a_fragment_with_two_machines_a_mid_program_tape_and_a_compound_round_trips_byte_equal() {
        let mut sim = Sim::empty();
        sim.glyphs
            .push(Some(Glyph::new(GlyphKind::Bonder, Hex::new(-2, 3), 5)));
        let mut arm = crate::sim::Arm::new(
            Hex::new(4, -1),
            2,
            vec![Instr::Grab, Instr::Move(4), Instr::Rot(Spin::Cw)],
        );
        arm.pc = 2;
        sim.arms.push(arm);
        let a = sim.spawn(Atom {
            kind: AtomKind::Base,
            pos: Hex::new(0, 1),
        });
        let b = sim.spawn(Atom {
            kind: AtomKind::Amber,
            pos: Hex::new(1, 1),
        });
        sim.bonds.push(Bond {
            a,
            b,
            kind: BondKind::Double,
        });

        let text = Fragment::of(&sim).to_string();
        let read = text.parse::<Fragment>().unwrap();
        assert_eq!(read.to_string(), text);
        assert_eq!(read.0.arms.len(), 1);
        assert_eq!(read.0.glyphs.iter().flatten().count(), 1);
        assert_eq!(read.0.atoms.iter().flatten().count(), 2);
        assert_eq!(read.0.arms[0].pc, 2);
        assert_eq!(read.0.arms[0].tape, sim.arms[0].tape);
        assert!(text.lines().any(|line| line.contains(" FwD 2")));
    }
}
