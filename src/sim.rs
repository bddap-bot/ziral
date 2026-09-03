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
pub enum AtomKind {
    Base,
}

pub const SACRIFICIAL: AtomKind = AtomKind::Base;

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
    pub stalled: bool,
}

impl Arm {
    pub fn new(pivot: Hex, dir: usize, tape: Vec<Instr>) -> Self {
        Arm {
            pivot,
            dir,
            tape,
            pc: 0,
            held: None,
            stalled: false,
        }
    }

    pub fn hand(&self) -> Hex {
        self.pivot.add(DIRS[self.dir])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphKind {
    Bonder,
    SecondBond,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub kind: GlyphKind,
    pub at: Hex,
    pub dir: usize,
}

impl Glyph {
    pub fn slots(&self) -> [Hex; 3] {
        [
            self.at,
            self.at.add(DIRS[self.dir % 6]),
            self.at.add(DIRS[(self.dir + 1) % 6]),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sim {
    pub sources: Vec<Hex>,
    pub outputs: Vec<Hex>,
    pub glyphs: Vec<Glyph>,
    pub arms: Vec<Arm>,
    pub atoms: Vec<Option<Atom>>,
    pub bonds: Vec<Bond>,
    pub tick: u64,
    pub delivered: u64,
}

impl Sim {
    pub fn empty() -> Self {
        Sim {
            sources: Vec::new(),
            outputs: Vec::new(),
            glyphs: Vec::new(),
            arms: Vec::new(),
            atoms: Vec::new(),
            bonds: Vec::new(),
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

    fn held_by_any(&self, id: usize) -> bool {
        self.arms.iter().any(|a| a.held == Some(id))
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

    fn destroy(&mut self, id: usize) {
        self.atoms[id] = None;
        self.bonds.retain(|x| x.a != id && x.b != id);
        for arm in &mut self.arms {
            if arm.held == Some(id) {
                arm.held = None;
            }
        }
    }

    pub fn step(&mut self) {
        for at in self.sources.clone() {
            if self.atom_at(at).is_none() {
                self.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: at,
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
            let done = self.exec(i, instr);
            let arm = &mut self.arms[i];
            arm.stalled = !done;
            if done {
                arm.pc = arm.pc.wrapping_add(1);
            }
        }
        for g in self.glyphs.clone() {
            self.fire(g);
        }
        for out in self.outputs.clone() {
            let Some(id) = self.atom_at(out) else {
                continue;
            };
            let comp = self.component(id);
            if !comp.iter().any(|id| self.held_by_any(*id)) {
                for id in comp {
                    self.destroy(id);
                }
                self.delivered += 1;
            }
        }
        self.tick += 1;
    }

    fn fire(&mut self, g: Glyph) {
        let ids: Vec<usize> = g.slots().iter().filter_map(|s| self.atom_at(*s)).collect();
        let [x, y, z] = ids[..] else { return };
        let Some(victim) = [x, y, z]
            .into_iter()
            .find(|id| self.atoms[*id].unwrap().kind == SACRIFICIAL)
        else {
            return;
        };
        let pair: Vec<usize> = [x, y, z].into_iter().filter(|id| *id != victim).collect();
        let (a, b) = (pair[0], pair[1]);
        match (g.kind, self.bond_between(a, b)) {
            (GlyphKind::Bonder, None) => {
                self.bonds.push(Bond {
                    a,
                    b,
                    kind: BondKind::Single,
                });
            }
            (GlyphKind::SecondBond, Some(i)) if self.bonds[i].kind == BondKind::Single => {
                self.bonds[i].kind = BondKind::Double;
            }
            _ => return,
        }
        self.destroy(victim);
    }

    fn exec(&mut self, i: usize, instr: Instr) -> bool {
        let hand = self.arms[i].hand();
        match instr {
            Instr::Wait => {}
            Instr::Grab => {
                let Some(id) = self.atom_at(hand) else {
                    return false;
                };
                if self.held_by_any(id) && self.arms[i].held != Some(id) {
                    return false;
                }
                self.arms[i].held = Some(id);
            }
            Instr::Drop => self.arms[i].held = None,
            Instr::RotCw | Instr::RotCcw => {
                let cw = instr == Instr::RotCw;
                let pivot = self.arms[i].pivot;
                if let Some(held) = self.arms[i].held {
                    let comp = self.component(held);
                    let shared = comp.iter().any(|id| {
                        self.arms
                            .iter()
                            .enumerate()
                            .any(|(j, a)| j != i && a.held == Some(*id))
                    });
                    if shared {
                        return false;
                    }
                    let moved: Vec<(usize, Hex)> = comp
                        .iter()
                        .map(|id| (*id, self.atoms[*id].unwrap().pos.rotate(pivot, cw)))
                        .collect();
                    let blocked = moved.iter().any(|(_, to)| {
                        self.atom_at(*to)
                            .is_some_and(|other| !comp.contains(&other))
                    });
                    if blocked {
                        return false;
                    }
                    for (id, to) in moved {
                        self.atoms[id].as_mut().unwrap().pos = to;
                    }
                }
                let arm = &mut self.arms[i];
                arm.dir = (arm.dir + if cw { 1 } else { 5 }) % 6;
            }
        }
        true
    }

    fn place(&mut self, other: &Sim, at: Hex) {
        self.sources.extend(other.sources.iter().map(|s| s.add(at)));
        self.outputs.extend(other.outputs.iter().map(|o| o.add(at)));
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
    sim.sources.push(Hex::new(1, 0));
    sim.outputs.push(Hex::new(0, 1));
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

    #[test]
    fn tape_wraps_and_rotation_carries_the_held_compound() {
        use Instr::*;
        let mut sim = bench(vec![Grab, RotCw], Vec::new());
        let a = put(&mut sim, 1, 0);
        let b = put(&mut sim, 1, -1);
        sim.bonds.push(Bond {
            a,
            b,
            kind: BondKind::Single,
        });
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
        assert!(sim.arms[0].stalled);
        assert_eq!(sim.arms[0].pc, 1);
        assert_eq!(sim.atoms[0].unwrap().pos, Hex::new(1, 0));
        sim.step();
        sim.step();
        assert!(sim.arms[0].stalled);
        sim.step();
        assert!(!sim.arms[0].stalled);
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
        assert!(sim.arms[0].stalled);
        assert_eq!(sim.arms[0].pc, 1);
        sim.step();
        assert!(sim.arms[0].stalled);
        sim.step();
        assert!(!sim.arms[0].stalled);
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
        assert!(!sim.arms[0].stalled);
        assert!(sim.arms[1].stalled);
        sim.step();
        assert!(!sim.arms[1].stalled);
        assert_eq!(sim.atoms[1].unwrap().pos, Hex::new(1, -1));
    }

    #[test]
    fn bonder_sacrifices_the_priority_slot_and_only_the_applicator_doubles() {
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(1, 0),
            dir: 1,
        };
        let second = Glyph {
            kind: GlyphKind::SecondBond,
            ..bonder
        };
        assert_eq!(
            bonder.slots(),
            [Hex::new(1, 0), Hex::new(2, -1), Hex::new(1, -1)]
        );
        let mut sim = bench(vec![Instr::Wait], vec![bonder, second]);
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
        sim.step();
        assert_eq!(sim.bonds.len(), 1);
        let again = put(&mut sim, 1, 0);
        sim.step();
        assert_eq!(sim.atoms[again], None);
        assert_eq!(sim.bonds[0].kind, BondKind::Double);
        sim.glyphs = vec![bonder];
        put(&mut sim, 1, 0);
        sim.step();
        assert_eq!(sim.live_atoms().count(), 3);
    }

    #[test]
    fn output_consumes_the_whole_compound_only_once_released() {
        let mut sim = bench(vec![Instr::Grab, Instr::Wait, Instr::Drop], Vec::new());
        sim.outputs = vec![Hex::new(1, 0)];
        let a = put(&mut sim, 1, 0);
        let b = put(&mut sim, 1, -1);
        sim.bonds.push(Bond {
            a,
            b,
            kind: BondKind::Single,
        });
        sim.step();
        sim.step();
        assert_eq!(sim.delivered, 0);
        assert_eq!(sim.live_atoms().count(), 2);
        sim.step();
        assert_eq!(sim.delivered, 1);
        assert!(sim.live_atoms().next().is_none());
        assert!(sim.bonds.is_empty());
    }

    #[test]
    fn preloaded_world_delivers_every_period_except_the_copy_that_stalls() {
        let mut sim = preloaded();
        for _ in 0..26 * 4 {
            sim.step();
        }
        assert_eq!(sim.delivered, 4 * (PLACEMENTS.len() as u64 - 1));
        let stalled: Vec<usize> = (0..sim.arms.len())
            .filter(|i| sim.arms[*i].stalled)
            .collect();
        assert_eq!(stalled, vec![sim.arms.len() - 2, sim.arms.len() - 1]);
        assert!(sim.atoms.len() < 40);
    }
}
