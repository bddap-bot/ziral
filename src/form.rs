use crate::sim::{
    Atom, AtomKind, Bond, BondKind, DIRS, GlyphKind, Hex, Instr, Item, MAX_COMPOUND_ATOMS, Machine,
    Sim, Spin, Tier,
};
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

pub const RECIPES: [(Item, &str); 23] = [
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
    (Item::Step, "B0,0"),
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

pub const CONVERTER_INPUTS: [(AtomKind, &str); 2] = [
    (AtomKind::Amber, "B0,0 B0,1 B1,0 0,0-0,1 0,0-1,0"),
    (AtomKind::Plum, "A0,0 B0,1 B1,1 0,0-0,1 0,1=1,1"),
];

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

impl FromStr for Form {
    type Err = String;

    fn from_str(text: &str) -> Result<Form, String> {
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
        Ok(Form::of(&sim))
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
}
