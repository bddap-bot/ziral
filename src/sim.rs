#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hex {
    pub q: i32,
    pub r: i32,
}

pub const DIRS: [Hex; 6] = [
    Hex { q: 1, r: 0 },
    Hex { q: 1, r: -1 },
    Hex { q: 0, r: -1 },
    Hex { q: -1, r: 0 },
    Hex { q: -1, r: 1 },
    Hex { q: 0, r: 1 },
];

pub const ORIGIN: Hex = Hex::new(0, 0);

impl Hex {
    pub const fn new(q: i32, r: i32) -> Self {
        Hex { q, r }
    }

    pub fn add(self, o: Hex) -> Hex {
        Hex::new(self.q + o.q, self.r + o.r)
    }

    pub fn sub(self, o: Hex) -> Hex {
        Hex::new(self.q - o.q, self.r - o.r)
    }

    pub fn rotate(self, pivot: Hex, cw: bool) -> Hex {
        let d = self.sub(pivot);
        let d = if cw {
            Hex::new(d.q + d.r, -d.q)
        } else {
            Hex::new(-d.r, d.q + d.r)
        };
        pivot.add(d)
    }

    pub fn turned(self, dir: usize) -> Hex {
        (0..dir % 6).fold(self, |h, _| h.rotate(ORIGIN, true))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instr {
    Grab,
    Drop,
    RotCw,
    RotCcw,
    Wait,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stall {
    Illegal,
    Hand(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomKind {
    Base,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Atom {
    pub kind: AtomKind,
    pub pos: Hex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BondKind {
    Single,
    Double,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bond {
    pub a: usize,
    pub b: usize,
    pub kind: BondKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arm {
    pub pivot: Hex,
    pub dir: usize,
    pub tape: Vec<Instr>,
    pub pc: usize,
    pub held: Option<usize>,
    pub stall: Option<Stall>,
}

impl Arm {
    pub fn new(pivot: Hex, dir: usize, tape: Vec<Instr>) -> Self {
        Arm {
            pivot,
            dir,
            tape,
            pc: 0,
            held: None,
            stall: None,
        }
    }

    pub fn hand(&self) -> Hex {
        self.pivot.add(DIRS[self.dir])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphKind {
    Source,
    Bonder,
    SecondBond,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub at: Hex,
    pub kind: AtomKind,
    pub consumed: bool,
    pub released: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub slots: &'static [Slot],
    pub before: &'static [(usize, usize, Option<BondKind>)],
    pub after: &'static [(usize, usize, BondKind)],
    pub whole: bool,
}

const fn base(at: Hex) -> Slot {
    Slot {
        at,
        kind: AtomKind::Base,
        consumed: false,
        released: false,
    }
}

const TRIANGLE: [Slot; 3] = [
    Slot {
        consumed: true,
        released: true,
        ..base(ORIGIN)
    },
    base(DIRS[0]),
    base(DIRS[1]),
];
const PAIR: [Slot; 2] = [
    Slot {
        consumed: true,
        released: true,
        ..base(ORIGIN)
    },
    Slot {
        consumed: true,
        released: true,
        ..base(DIRS[0])
    },
];
const ONE: [Slot; 1] = [base(ORIGIN)];

impl GlyphKind {
    pub const fn rule(self) -> Rule {
        match self {
            GlyphKind::Source => Rule {
                slots: &ONE,
                before: &[],
                after: &[],
                whole: false,
            },
            GlyphKind::Bonder => Rule {
                slots: &TRIANGLE,
                before: &[(1, 2, None)],
                after: &[(1, 2, BondKind::Single)],
                whole: false,
            },
            GlyphKind::SecondBond => Rule {
                slots: &TRIANGLE,
                before: &[(1, 2, Some(BondKind::Single))],
                after: &[(1, 2, BondKind::Double)],
                whole: false,
            },
            GlyphKind::Output => Rule {
                slots: &PAIR,
                before: &[(0, 1, Some(BondKind::Double))],
                after: &[],
                whole: true,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub kind: GlyphKind,
    pub at: Hex,
    pub dir: usize,
}

impl Glyph {
    pub fn slots(&self) -> impl Iterator<Item = Hex> + '_ {
        self.kind
            .rule()
            .slots
            .iter()
            .map(move |s| self.at.add(s.at.turned(self.dir)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sim {
    pub glyphs: Vec<Glyph>,
    pub arms: Vec<Arm>,
    pub atoms: Vec<Option<Atom>>,
    pub bonds: Vec<Bond>,
    pub torn: Vec<(Hex, Hex, BondKind)>,
    pub tick: u64,
    pub delivered: u64,
}

impl Sim {
    pub fn empty() -> Self {
        Sim {
            glyphs: Vec::new(),
            arms: Vec::new(),
            atoms: Vec::new(),
            bonds: Vec::new(),
            torn: Vec::new(),
            tick: 0,
            delivered: 0,
        }
    }

    pub fn atom_at(&self, at: Hex) -> Option<usize> {
        self.atoms
            .iter()
            .position(|a| a.is_some_and(|a| a.pos == at))
    }

    pub fn live_atoms(&self) -> impl Iterator<Item = (usize, Atom)> + '_ {
        self.atoms
            .iter()
            .enumerate()
            .filter_map(|(i, a)| a.map(|a| (i, a)))
    }

    pub fn bond_between(&self, a: usize, b: usize) -> Option<usize> {
        self.bonds
            .iter()
            .position(|x| (x.a == a && x.b == b) || (x.a == b && x.b == a))
    }

    pub fn component(&self, start: usize) -> Vec<usize> {
        let mut seen = vec![start];
        let mut i = 0;
        while i < seen.len() {
            let cur = seen[i];
            for x in &self.bonds {
                let other = if x.a == cur {
                    x.b
                } else if x.b == cur {
                    x.a
                } else {
                    continue;
                };
                if !seen.contains(&other) {
                    seen.push(other);
                }
            }
            i += 1;
        }
        seen
    }

    fn holder(&self, id: usize) -> Option<usize> {
        self.arms.iter().position(|a| a.held == Some(id))
    }

    fn component_released(&self, id: usize) -> bool {
        !self.component(id).iter().any(|a| self.holder(*a).is_some())
    }

    pub fn spawn(&mut self, atom: Atom) -> usize {
        match self.atoms.iter().position(|a| a.is_none()) {
            Some(free) => {
                self.atoms[free] = Some(atom);
                free
            }
            None => {
                self.atoms.push(Some(atom));
                self.atoms.len() - 1
            }
        }
    }

    pub fn other_hand(&self, i: usize) -> Option<usize> {
        self.component(self.arms[i].held?)
            .into_iter()
            .filter_map(|id| self.holder(id))
            .find(|j| *j != i)
    }

    fn consume(&mut self, ids: &[usize]) {
        let gone = |id: usize| ids.contains(&id);
        let pos = |id: usize| self.atoms[id].unwrap().pos;
        let torn = self
            .bonds
            .iter()
            .filter_map(|x| match (gone(x.a), gone(x.b)) {
                (false, true) => Some((pos(x.a), pos(x.b), x.kind)),
                (true, false) => Some((pos(x.b), pos(x.a), x.kind)),
                _ => None,
            });
        self.torn.extend(torn);
        self.bonds.retain(|x| !gone(x.a) && !gone(x.b));
        for id in ids {
            self.atoms[*id] = None;
        }
        for arm in &mut self.arms {
            if arm.held.is_some_and(gone) {
                arm.held = None;
            }
        }
    }

    pub fn step(&mut self) {
        self.torn.clear();
        for i in 0..self.glyphs.len() {
            let g = self.glyphs[i];
            if g.kind == GlyphKind::Source && self.atom_at(g.at).is_none() {
                self.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: g.at,
                });
            }
        }
        for i in 0..self.arms.len() {
            let arm = &self.arms[i];
            let instr = if arm.tape.is_empty() {
                Instr::Wait
            } else {
                arm.tape[arm.pc % arm.tape.len()]
            };
            let stall = self.exec(i, instr).err();
            let arm = &mut self.arms[i];
            arm.stall = stall;
            if stall.is_none() {
                arm.pc = arm.pc.wrapping_add(1);
            }
        }
        for i in 0..self.glyphs.len() {
            self.fire(self.glyphs[i]);
        }
        self.tick += 1;
    }

    fn matched(&self, g: Glyph) -> Option<Vec<usize>> {
        let rule = g.kind.rule();
        let ids: Vec<usize> = g
            .slots()
            .zip(rule.slots)
            .map(|(at, slot)| {
                self.atom_at(at)
                    .filter(|id| self.atoms[*id].unwrap().kind == slot.kind)
            })
            .collect::<Option<_>>()?;
        let bonded = |a: usize, b: usize| self.bond_between(a, b).map(|i| self.bonds[i].kind);
        if rule
            .before
            .iter()
            .any(|(a, b, want)| bonded(ids[*a], ids[*b]) != *want)
        {
            return None;
        }
        if rule
            .slots
            .iter()
            .zip(&ids)
            .any(|(slot, id)| slot.released && !self.component_released(*id))
        {
            return None;
        }
        if rule.whole {
            let comp = self.component(ids[0]);
            if comp.len() != ids.len() || comp.iter().any(|id| !ids.contains(id)) {
                return None;
            }
        }
        Some(ids)
    }

    fn fire(&mut self, g: Glyph) {
        let rule = g.kind.rule();
        let Some(ids) = self.matched(g) else { return };
        for (a, b, kind) in rule.after {
            let (a, b) = (ids[*a], ids[*b]);
            match self.bond_between(a, b) {
                Some(i) => self.bonds[i].kind = *kind,
                None => self.bonds.push(Bond { a, b, kind: *kind }),
            }
        }
        let consumed: Vec<usize> = rule
            .slots
            .iter()
            .zip(&ids)
            .filter(|(slot, _)| slot.consumed)
            .map(|(_, id)| *id)
            .collect();
        self.consume(&consumed);
        if g.kind == GlyphKind::Output {
            self.delivered += 1;
        }
    }

    fn exec(&mut self, i: usize, instr: Instr) -> Result<(), Stall> {
        let hand = self.arms[i].hand();
        match instr {
            Instr::Wait => {}
            Instr::Grab => {
                let id = self.atom_at(hand).ok_or(Stall::Illegal)?;
                if self.holder(id).is_some_and(|j| j != i) {
                    return Err(Stall::Illegal);
                }
                self.arms[i].held = Some(id);
            }
            Instr::Drop => self.arms[i].held = None,
            Instr::RotCw | Instr::RotCcw => {
                let cw = instr == Instr::RotCw;
                let pivot = self.arms[i].pivot;
                if let Some(held) = self.arms[i].held {
                    if let Some(j) = self.other_hand(i) {
                        return Err(Stall::Hand(j));
                    }
                    let comp = self.component(held);
                    let moved: Vec<(usize, Hex)> = comp
                        .iter()
                        .map(|id| (*id, self.atoms[*id].unwrap().pos.rotate(pivot, cw)))
                        .collect();
                    let blocked = moved.iter().any(|(_, to)| {
                        self.atom_at(*to)
                            .is_some_and(|other| !comp.contains(&other))
                    });
                    if blocked {
                        return Err(Stall::Illegal);
                    }
                    for (id, to) in moved {
                        self.atoms[id].as_mut().unwrap().pos = to;
                    }
                }
                let arm = &mut self.arms[i];
                arm.dir = (arm.dir + if cw { 1 } else { 5 }) % 6;
            }
        }
        Ok(())
    }

    fn place(&mut self, other: &Sim, at: Hex) {
        self.glyphs.extend(other.glyphs.iter().map(|g| Glyph {
            at: g.at.add(at),
            ..*g
        }));
        self.arms.extend(other.arms.iter().map(|a| Arm {
            pivot: a.pivot.add(at),
            ..a.clone()
        }));
    }
}

pub const PLACEMENTS: [Hex; 6] = [
    Hex::new(0, 0),
    Hex::new(11, 0),
    Hex::new(22, 0),
    Hex::new(3, -9),
    Hex::new(14, -9),
    Hex::new(8, 9),
];

fn layout() -> Sim {
    use Instr::*;
    let supply = [Grab, RotCw, RotCw, Drop, RotCcw, RotCcw];
    let mut build: Vec<Instr> = supply.repeat(3);
    build.extend([Grab, RotCw, RotCw, RotCw, RotCw, Drop, RotCw, RotCw]);
    let mut ferry = vec![Wait; 4];
    ferry.extend([
        Grab, RotCw, Drop, RotCcw, Wait, Wait, Grab, RotCcw, Drop, RotCw,
    ]);
    ferry.resize(build.len(), Wait);
    let mut sim = Sim::empty();
    sim.glyphs.push(Glyph {
        kind: GlyphKind::Source,
        at: Hex::new(1, 0),
        dir: 0,
    });
    sim.glyphs.push(Glyph {
        kind: GlyphKind::Output,
        at: Hex::new(0, 1),
        dir: 3,
    });
    sim.glyphs.push(Glyph {
        kind: GlyphKind::Bonder,
        at: Hex::new(1, -2),
        dir: 4,
    });
    sim.glyphs.push(Glyph {
        kind: GlyphKind::SecondBond,
        at: Hex::new(-1, -1),
        dir: 5,
    });
    sim.arms.push(Arm::new(Hex::new(0, 0), 0, build));
    sim.arms.push(Arm::new(Hex::new(0, -2), 5, ferry));
    sim
}

pub fn preloaded() -> Sim {
    let one = layout();
    let mut world = Sim::empty();
    for at in PLACEMENTS {
        world.place(&one, at);
    }
    world.spawn(Atom {
        kind: AtomKind::Base,
        pos: Hex::new(-1, 0).add(PLACEMENTS[PLACEMENTS.len() - 1]),
    });
    world
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bench(tape: Vec<Instr>, glyphs: Vec<Glyph>) -> Sim {
        let mut sim = Sim::empty();
        sim.glyphs = glyphs;
        sim.arms.push(Arm::new(Hex::new(0, 0), 0, tape));
        sim
    }

    fn put(sim: &mut Sim, q: i32, r: i32) -> usize {
        sim.spawn(Atom {
            kind: AtomKind::Base,
            pos: Hex::new(q, r),
        })
    }

    fn bond(sim: &mut Sim, a: usize, b: usize, kind: BondKind) {
        sim.bonds.push(Bond { a, b, kind });
    }

    #[test]
    fn tape_wraps_and_rotation_carries_the_held_compound() {
        use Instr::*;
        let mut sim = bench(vec![Grab, RotCw], Vec::new());
        let a = put(&mut sim, 1, 0);
        let b = put(&mut sim, 1, -1);
        bond(&mut sim, a, b, BondKind::Single);
        for _ in 0..4 {
            sim.step();
        }
        assert_eq!(sim.arms[0].dir, 2);
        assert_eq!(sim.atoms[a].unwrap().pos, Hex::new(0, -1));
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(-1, 0));
    }

    #[test]
    fn a_rotation_into_an_occupied_cell_stalls_until_it_clears() {
        use Instr::*;
        let mut sim = bench(vec![Grab, RotCw], Vec::new());
        sim.arms
            .push(Arm::new(Hex::new(2, -2), 4, vec![Wait, Wait, Grab, RotCw]));
        put(&mut sim, 1, 0);
        put(&mut sim, 1, -1);
        sim.step();
        sim.step();
        assert!(sim.arms[0].stall.is_some());
        assert_eq!(sim.arms[0].pc, 1);
        assert_eq!(sim.atoms[0].unwrap().pos, Hex::new(1, 0));
        sim.step();
        sim.step();
        assert!(sim.arms[0].stall.is_some());
        sim.step();
        assert!(sim.arms[0].stall.is_none());
        assert_eq!(sim.atoms[0].unwrap().pos, Hex::new(1, -1));
    }

    #[test]
    fn a_grab_of_a_held_atom_stalls_until_it_is_released() {
        use Instr::*;
        let mut sim = bench(vec![Wait, Grab, RotCw], Vec::new());
        sim.arms
            .push(Arm::new(Hex::new(2, 0), 3, vec![Grab, Wait, Drop, Wait]));
        put(&mut sim, 1, 0);
        sim.step();
        sim.step();
        assert!(sim.arms[0].stall.is_some());
        assert_eq!(sim.arms[0].pc, 1);
        sim.step();
        assert!(sim.arms[0].stall.is_some());
        sim.step();
        assert!(sim.arms[0].stall.is_none());
        assert_eq!(sim.arms[0].held, Some(0));
        assert_eq!(sim.arms[1].held, None);
    }

    #[test]
    fn two_arms_contending_for_one_cell_resolve_in_arm_order() {
        use Instr::*;
        let mut sim = bench(vec![Grab, RotCw, RotCw, Wait], Vec::new());
        sim.arms
            .push(Arm::new(Hex::new(2, -2), 3, vec![Grab, RotCw]));
        put(&mut sim, 1, 0);
        put(&mut sim, 1, -2);
        sim.step();
        sim.step();
        assert_eq!(sim.atoms[0].unwrap().pos, Hex::new(1, -1));
        assert_eq!(sim.atoms[1].unwrap().pos, Hex::new(1, -2));
        assert!(sim.arms[0].stall.is_none());
        assert!(sim.arms[1].stall.is_some());
        sim.step();
        assert!(sim.arms[1].stall.is_none());
        assert_eq!(sim.atoms[1].unwrap().pos, Hex::new(1, -1));
    }

    #[test]
    fn a_glyph_turns_with_its_dir_and_a_glyph_reads_its_own_slots() {
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(1, 0),
            dir: 1,
        };
        assert_eq!(
            bonder.slots().collect::<Vec<_>>(),
            [Hex::new(1, 0), Hex::new(2, -1), Hex::new(1, -1)]
        );
        let mut sim = bench(vec![Instr::Wait], vec![bonder]);
        let first = put(&mut sim, 1, 0);
        let a = put(&mut sim, 2, -1);
        let b = put(&mut sim, 1, -1);
        sim.step();
        assert_eq!(sim.atoms[first], None);
        assert_eq!(
            sim.bonds,
            vec![Bond {
                a,
                b,
                kind: BondKind::Single
            }]
        );
        put(&mut sim, 1, 0);
        sim.step();
        assert_eq!(sim.live_atoms().count(), 3);
        assert_eq!(sim.bonds.len(), 1);
    }

    #[test]
    fn machines_never_collide_so_glyphs_stack_on_one_hex() {
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(1, 0),
            dir: 1,
        };
        let second = Glyph {
            kind: GlyphKind::SecondBond,
            ..bonder
        };
        let mut sim = bench(vec![Instr::Wait], vec![bonder, second]);
        sim.arms
            .push(Arm::new(Hex::new(0, 0), 0, vec![Instr::Wait]));
        put(&mut sim, 1, 0);
        put(&mut sim, 2, -1);
        put(&mut sim, 1, -1);
        sim.step();
        assert_eq!(sim.bonds[0].kind, BondKind::Single);
        assert_eq!(sim.live_atoms().count(), 2);
        put(&mut sim, 1, 0);
        sim.step();
        assert_eq!(sim.bonds[0].kind, BondKind::Double);
        assert_eq!(sim.live_atoms().count(), 2);
    }

    #[test]
    fn an_output_takes_only_the_exact_shape_atoms_and_bonds_once_released() {
        let output = Glyph {
            kind: GlyphKind::Output,
            at: Hex::new(1, 0),
            dir: 5,
        };
        assert_eq!(
            output.slots().collect::<Vec<_>>(),
            [Hex::new(1, 0), Hex::new(1, 1)]
        );
        let mut sim = bench(vec![Instr::Wait], vec![output]);
        let a = put(&mut sim, 1, 0);
        let b = put(&mut sim, 1, 1);
        bond(&mut sim, a, b, BondKind::Single);
        sim.step();
        assert_eq!(sim.delivered, 0);
        sim.bonds[0].kind = BondKind::Double;
        let c = put(&mut sim, 2, 0);
        bond(&mut sim, b, c, BondKind::Single);
        sim.step();
        assert_eq!(sim.delivered, 0);
        sim.consume(&[c]);
        sim.arms[0].tape = vec![Instr::Grab, Instr::Wait, Instr::Drop];
        sim.arms[0].pc = 0;
        sim.step();
        sim.step();
        assert_eq!(sim.delivered, 0);
        assert_eq!(sim.live_atoms().count(), 2);
        sim.step();
        assert_eq!(sim.delivered, 1);
        assert!(sim.live_atoms().next().is_none());
        assert!(sim.bonds.is_empty());
        assert!(sim.torn.is_empty());
    }

    #[test]
    fn a_bonder_waits_for_its_sacrificial_atom_to_be_released() {
        use Instr::*;
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(1, 0),
            dir: 1,
        };
        let mut sim = bench(vec![Grab, Wait, Drop], vec![bonder]);
        let sacrificial = put(&mut sim, 1, 0);
        put(&mut sim, 2, -1);
        put(&mut sim, 1, -1);
        sim.step();
        sim.step();
        assert_eq!(sim.arms[0].held, Some(sacrificial));
        assert_eq!(sim.live_atoms().count(), 3);
        assert!(sim.bonds.is_empty());
        sim.step();
        assert_eq!(sim.atoms[sacrificial], None);
        assert_eq!(sim.bonds.len(), 1);
    }

    #[test]
    fn a_second_bond_waits_for_its_sacrificial_atom_to_be_released() {
        use Instr::*;
        let second = Glyph {
            kind: GlyphKind::SecondBond,
            at: Hex::new(1, 0),
            dir: 1,
        };
        let mut sim = bench(vec![Grab, Wait, Drop], vec![second]);
        let sacrificial = put(&mut sim, 1, 0);
        let a = put(&mut sim, 2, -1);
        let b = put(&mut sim, 1, -1);
        bond(&mut sim, a, b, BondKind::Single);
        sim.step();
        sim.step();
        assert_eq!(sim.bonds[0].kind, BondKind::Single);
        assert_eq!(sim.live_atoms().count(), 3);
        sim.step();
        assert_eq!(sim.bonds[0].kind, BondKind::Double);
        assert_eq!(sim.atoms[sacrificial], None);
    }

    #[test]
    fn a_bonder_fires_on_atoms_an_arm_holds_and_the_arm_keeps_the_compound() {
        use Instr::*;
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(1, -1),
            dir: 0,
        };
        assert_eq!(
            bonder.slots().collect::<Vec<_>>(),
            [Hex::new(1, -1), Hex::new(2, -1), Hex::new(2, -2)]
        );
        let mut sim = bench(vec![Grab, RotCw], vec![bonder]);
        sim.arms[0].pivot = Hex::new(1, 0);
        sim.arms[0].dir = 1;
        let sacrificial = put(&mut sim, 1, -1);
        let held = put(&mut sim, 2, -1);
        let other = put(&mut sim, 2, -2);
        sim.step();
        assert_eq!(sim.arms[0].held, Some(held));
        assert_eq!(sim.atoms[sacrificial], None);
        assert_eq!(
            sim.bonds,
            vec![Bond {
                a: held,
                b: other,
                kind: BondKind::Single
            }]
        );
        sim.step();
        assert_eq!(sim.atoms[held].unwrap().pos, Hex::new(1, -1));
        assert_eq!(sim.atoms[other].unwrap().pos, Hex::new(0, -1));
    }

    #[test]
    fn a_sacrificial_atom_bonded_into_a_held_compound_is_not_released() {
        use Instr::*;
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(1, 0),
            dir: 1,
        };
        let mut sim = bench(vec![Grab, Wait, Drop], vec![bonder]);
        sim.arms[0].pivot = Hex::new(3, 0);
        sim.arms[0].dir = 3;
        let sacrificial = put(&mut sim, 1, 0);
        put(&mut sim, 2, -1);
        put(&mut sim, 1, -1);
        let tail = put(&mut sim, 2, 0);
        bond(&mut sim, sacrificial, tail, BondKind::Single);
        sim.step();
        sim.step();
        assert_eq!(sim.arms[0].held, Some(tail));
        assert_eq!(sim.live_atoms().count(), 4);
        sim.step();
        assert_eq!(sim.atoms[sacrificial], None);
        assert_eq!(sim.live_atoms().count(), 3);
        assert_eq!(sim.bonds.len(), 1);
    }

    #[test]
    fn a_rotate_under_two_hands_stalls_and_names_the_other_hand_until_it_drops() {
        use Instr::*;
        let mut sim = bench(vec![Grab, RotCw], Vec::new());
        sim.arms
            .push(Arm::new(Hex::new(2, -2), 4, vec![Grab, Wait, Drop, Wait]));
        let a = put(&mut sim, 1, 0);
        let b = put(&mut sim, 1, -1);
        bond(&mut sim, a, b, BondKind::Single);
        sim.step();
        assert_eq!(sim.other_hand(0), Some(1));
        assert_eq!(sim.other_hand(1), Some(0));
        sim.step();
        assert_eq!(sim.arms[0].stall, Some(Stall::Hand(1)));
        assert_eq!(sim.atoms[a].unwrap().pos, Hex::new(1, 0));
        sim.step();
        assert_eq!(sim.arms[0].stall, Some(Stall::Hand(1)));
        assert_eq!(sim.other_hand(0), None);
        sim.step();
        assert!(sim.arms[0].stall.is_none());
        assert_eq!(sim.atoms[a].unwrap().pos, Hex::new(1, -1));
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(0, -1));
    }

    #[test]
    fn a_bonder_tears_its_sacrificial_atom_out_of_an_unheld_compound_for_one_tick() {
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(1, 0),
            dir: 1,
        };
        let mut sim = bench(vec![Instr::Wait], vec![bonder]);
        let sacrificial = put(&mut sim, 1, 0);
        let a = put(&mut sim, 2, -1);
        let b = put(&mut sim, 1, -1);
        let tail = put(&mut sim, 2, 0);
        let end = put(&mut sim, 3, 0);
        bond(&mut sim, sacrificial, tail, BondKind::Double);
        bond(&mut sim, tail, end, BondKind::Single);
        sim.step();
        assert_eq!(sim.atoms[sacrificial], None);
        assert_eq!(
            sim.torn,
            vec![(Hex::new(2, 0), Hex::new(1, 0), BondKind::Double)]
        );
        assert_eq!(
            sim.bonds,
            vec![
                Bond {
                    a: tail,
                    b: end,
                    kind: BondKind::Single
                },
                Bond {
                    a,
                    b,
                    kind: BondKind::Single
                }
            ]
        );
        sim.step();
        assert!(sim.torn.is_empty());
    }

    #[test]
    fn an_output_turned_away_from_the_compound_ignores_it() {
        let mut sim = bench(vec![Instr::Wait], Vec::new());
        let a = put(&mut sim, 1, 0);
        let b = put(&mut sim, 1, 1);
        bond(&mut sim, a, b, BondKind::Double);
        for dir in 0..6 {
            sim.glyphs = vec![Glyph {
                kind: GlyphKind::Output,
                at: Hex::new(1, 0),
                dir,
            }];
            sim.step();
            assert_eq!(sim.delivered, u64::from(dir == 5), "dir {dir}");
        }
    }

    #[test]
    fn preloaded_world_delivers_every_period_except_the_copy_that_stalls() {
        let mut sim = preloaded();
        for _ in 0..26 * 4 {
            sim.step();
        }
        assert_eq!(sim.delivered, 4 * (PLACEMENTS.len() as u64 - 1));
        let stalled: Vec<usize> = (0..sim.arms.len())
            .filter(|i| sim.arms[*i].stall.is_some())
            .collect();
        assert_eq!(stalled, vec![sim.arms.len() - 2, sim.arms.len() - 1]);
        assert!(sim.atoms.len() < 40);
    }
}
