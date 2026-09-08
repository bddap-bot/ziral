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

    pub fn scramble(self) -> u32 {
        let mut x = (self.q as u32).wrapping_mul(0x9E37_79B1);
        x ^= x >> 15;
        x ^= (self.r as u32).wrapping_mul(0x85EB_CA77);
        x = x.wrapping_mul(0x2C1B_3C6D);
        x ^ (x >> 12)
    }

    pub fn rotate(self, pivot: Hex, spin: Spin) -> Hex {
        let d = self.sub(pivot);
        let d = match spin {
            Spin::Cw => Hex::new(d.q + d.r, -d.q),
            Spin::Ccw => Hex::new(-d.r, d.q + d.r),
        };
        pivot.add(d)
    }

    pub fn turned(self, dir: usize) -> Hex {
        (0..dir % 6).fold(self, |h, _| h.rotate(ORIGIN, Spin::Cw))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spin {
    Cw,
    Ccw,
}

impl Spin {
    pub fn turn(self, dir: usize) -> usize {
        let step = match self {
            Spin::Cw => 1,
            Spin::Ccw => 5,
        };
        (dir + step) % 6
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instr {
    Grab,
    Drop,
    Rot(Spin),
    Pivot(Spin),
    Move(usize),
    Wait,
}

impl Instr {
    pub fn posed(self, pivot: Hex, dir: usize) -> (Hex, usize) {
        match self {
            Instr::Rot(spin) => (pivot, spin.turn(dir)),
            Instr::Move(d) => (pivot.add(DIRS[d]), dir),
            _ => (pivot, dir),
        }
    }
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

impl AtomKind {
    pub const ALL: [AtomKind; 1] = [AtomKind::Base];
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

impl BondKind {
    pub const ALL: [BondKind; 2] = [BondKind::Single, BondKind::Double];
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
    pub holding: bool,
    pub stall: Option<Stall>,
}

impl Arm {
    pub fn new(pivot: Hex, dir: usize, tape: Vec<Instr>) -> Self {
        Arm {
            pivot,
            dir,
            tape,
            pc: 0,
            holding: false,
            stall: None,
        }
    }

    pub fn hand(&self) -> Hex {
        self.pivot.add(DIRS[self.dir])
    }

    pub fn swung(&self, after: &Arm) -> Option<(Hex, Spin)> {
        if self.tape.is_empty() || after.pc == self.pc {
            return None;
        }
        match self.tape[self.pc % self.tape.len()] {
            Instr::Rot(spin) => Some((self.pivot, spin)),
            Instr::Pivot(spin) => Some((self.hand(), spin)),
            _ => None,
        }
    }

    pub fn cells(&self) -> [Hex; 2] {
        [self.pivot, self.hand()]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphKind {
    Source,
    Bonder,
    SecondBond,
    Output,
    Cleanup,
}

impl GlyphKind {
    pub const ALL: [GlyphKind; 5] = [
        GlyphKind::Source,
        GlyphKind::Bonder,
        GlyphKind::SecondBond,
        GlyphKind::Output,
        GlyphKind::Cleanup,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub at: Hex,
    pub kind: AtomKind,
    pub consumed: bool,
    pub lone: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub slots: &'static [Slot],
    pub before: &'static [(usize, usize, Option<BondKind>)],
    pub after: &'static [(usize, usize, BondKind)],
    pub whole: bool,
    pub spent: bool,
}

const fn base(at: Hex) -> Slot {
    Slot {
        at,
        kind: AtomKind::Base,
        consumed: false,
        lone: false,
    }
}

const fn consumed(at: Hex) -> Slot {
    Slot {
        consumed: true,
        ..base(at)
    }
}

const SECOND_BOND: [Slot; 3] = [
    Slot {
        lone: true,
        ..consumed(ORIGIN)
    },
    base(DIRS[0]),
    base(DIRS[1]),
];
const BONDER: [Slot; 2] = [base(ORIGIN), base(DIRS[0])];
const OUTPUT: [Slot; 2] = [consumed(ORIGIN), consumed(DIRS[0])];
const SOURCE: [Slot; 1] = [base(ORIGIN)];
const CLEANUP: [Slot; 1] = [consumed(ORIGIN)];

const fn plain(slots: &'static [Slot]) -> Rule {
    Rule {
        slots,
        before: &[],
        after: &[],
        whole: false,
        spent: false,
    }
}

impl GlyphKind {
    pub const fn rule(self) -> Rule {
        match self {
            GlyphKind::Source => plain(&SOURCE),
            GlyphKind::Bonder => Rule {
                before: &[(0, 1, None)],
                after: &[(0, 1, BondKind::Single)],
                ..plain(&BONDER)
            },
            GlyphKind::SecondBond => Rule {
                before: &[(1, 2, Some(BondKind::Single))],
                after: &[(1, 2, BondKind::Double)],
                ..plain(&SECOND_BOND)
            },
            GlyphKind::Output => Rule {
                before: &[(0, 1, Some(BondKind::Double))],
                whole: true,
                ..plain(&OUTPUT)
            },
            GlyphKind::Cleanup => Rule {
                spent: true,
                ..plain(&CLEANUP)
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

pub const MAX_COMPOUND_ATOMS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sim {
    pub glyphs: Vec<Option<Glyph>>,
    pub arms: Vec<Arm>,
    pub atoms: Vec<Option<Atom>>,
    pub bonds: Vec<Bond>,
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
            tick: 0,
            delivered: 0,
        }
    }

    pub fn atom_at(&self, at: Hex) -> Option<usize> {
        self.atoms
            .iter()
            .position(|a| a.is_some_and(|a| a.pos == at))
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

    fn held(&self, i: usize) -> Option<usize> {
        let arm = &self.arms[i];
        arm.holding.then(|| self.atom_at(arm.hand())).flatten()
    }

    pub fn spawn(&mut self, atom: Atom) -> usize {
        seat(&mut self.atoms, atom)
    }

    fn other_hand(&self, i: usize) -> Option<usize> {
        let comp = self.component(self.held(i)?);
        (0..self.arms.len()).find(|j| *j != i && self.held(*j).is_some_and(|id| comp.contains(&id)))
    }

    fn consume(&mut self, ids: &[usize]) {
        let gone = |id: usize| ids.contains(&id);
        self.bonds.retain(|x| !gone(x.a) && !gone(x.b));
        for id in ids {
            self.atoms[*id] = None;
        }
    }

    pub fn replay(&self, ticks: u64) -> Sim {
        let mut sim = self.clone();
        for _ in 0..ticks {
            sim.step();
        }
        sim
    }

    pub fn step(&mut self) {
        for i in 0..self.glyphs.len() {
            let Some(g) = self.glyphs[i] else { continue };
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
            if self.act(i, instr) {
                self.arms[i].pc = self.arms[i].pc.wrapping_add(1);
            }
        }
        for i in 0..self.glyphs.len() {
            let Some(g) = self.glyphs[i] else { continue };
            if self.fire(g) && g.kind.rule().spent {
                self.glyphs[i] = None;
            }
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
                    .filter(|id| !slot.lone || self.bonds.iter().all(|x| x.a != *id && x.b != *id))
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
        if rule.whole {
            let comp = self.component(ids[0]);
            if comp.len() != ids.len() || comp.iter().any(|id| !ids.contains(id)) {
                return None;
            }
        }
        for (a, b, _) in rule.after {
            let (a, b) = (ids[*a], ids[*b]);
            if bonded(a, b).is_some() {
                continue;
            }
            let comp = self.component(a);
            if !comp.contains(&b) && comp.len() + self.component(b).len() > MAX_COMPOUND_ATOMS {
                return None;
            }
        }
        Some(ids)
    }

    fn fire(&mut self, g: Glyph) -> bool {
        let rule = g.kind.rule();
        let Some(ids) = self.matched(g) else {
            return false;
        };
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
        true
    }

    pub fn act(&mut self, i: usize, instr: Instr) -> bool {
        let stall = self.exec(i, instr).err();
        self.arms[i].stall = stall;
        stall.is_none()
    }

    fn exec(&mut self, i: usize, instr: Instr) -> Result<(), Stall> {
        let arm = &self.arms[i];
        let hand = arm.hand();
        let (pivot, dir) = instr.posed(arm.pivot, arm.dir);
        match instr {
            Instr::Wait => {}
            Instr::Grab => {
                self.atom_at(hand).ok_or(Stall::Illegal)?;
                self.arms[i].holding = true;
            }
            Instr::Drop => self.arms[i].holding = false,
            Instr::Rot(spin) => self.carry(i, pivot, |p| p.rotate(pivot, spin))?,
            Instr::Pivot(spin) => self.carry(i, pivot, |p| p.rotate(hand, spin))?,
            Instr::Move(d) => self.carry(i, pivot, |p| p.add(DIRS[d]))?,
        }
        let arm = &mut self.arms[i];
        (arm.pivot, arm.dir) = (pivot, dir);
        Ok(())
    }

    fn carry(&mut self, i: usize, pivot: Hex, to: impl Fn(Hex) -> Hex) -> Result<(), Stall> {
        let comp = match self.held(i) {
            Some(held) => {
                if let Some(j) = self.other_hand(i) {
                    return Err(Stall::Hand(j));
                }
                self.component(held)
            }
            None => Vec::new(),
        };
        let moved: Vec<(usize, Hex)> = comp
            .iter()
            .map(|id| (*id, to(self.atoms[*id].unwrap().pos)))
            .collect();
        let free = |at: Hex| {
            self.atom_at(at).is_none_or(|other| comp.contains(&other))
                && !self
                    .arms
                    .iter()
                    .enumerate()
                    .any(|(j, a)| j != i && a.pivot == at)
        };
        let stepped = pivot != self.arms[i].pivot;
        let seated = self
            .glyphs
            .iter()
            .flatten()
            .any(|g| g.slots().any(|s| s == pivot));
        if (stepped && (seated || !free(pivot)))
            || moved.iter().any(|(_, at)| !free(*at) || *at == pivot)
        {
            return Err(Stall::Illegal);
        }
        for (id, at) in moved {
            self.atoms[id].as_mut().unwrap().pos = at;
        }
        Ok(())
    }

    pub fn place(&mut self, other: &Sim, at: Hex) {
        self.glyphs.extend(other.glyphs.iter().flatten().map(|g| {
            Some(Glyph {
                at: g.at.add(at),
                ..*g
            })
        }));
        self.arms.extend(other.arms.iter().map(|a| Arm {
            pivot: a.pivot.add(at),
            ..a.clone()
        }));
        let ids: Vec<Option<usize>> = other
            .atoms
            .iter()
            .map(|atom| {
                atom.map(|atom| {
                    self.spawn(Atom {
                        pos: atom.pos.add(at),
                        ..atom
                    })
                })
            })
            .collect();
        self.bonds.extend(other.bonds.iter().map(|bond| Bond {
            a: ids[bond.a].unwrap(),
            b: ids[bond.b].unwrap(),
            ..*bond
        }));
    }
}

pub fn seat<T>(slots: &mut Vec<Option<T>>, x: T) -> usize {
    match slots.iter().position(Option::is_none) {
        Some(free) => {
            slots[free] = Some(x);
            free
        }
        None => {
            slots.push(Some(x));
            slots.len() - 1
        }
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

pub fn layout() -> Sim {
    use Instr::*;
    let supply = [
        Grab,
        Rot(Spin::Cw),
        Rot(Spin::Cw),
        Drop,
        Rot(Spin::Ccw),
        Rot(Spin::Ccw),
    ];
    let mut build: Vec<Instr> = supply.repeat(2);
    build.extend([
        Grab,
        Rot(Spin::Cw),
        Rot(Spin::Cw),
        Rot(Spin::Cw),
        Rot(Spin::Cw),
        Drop,
        Rot(Spin::Cw),
        Rot(Spin::Cw),
    ]);
    let mut ferry = vec![Wait; 4];
    ferry.extend([Grab, Rot(Spin::Ccw), Drop, Rot(Spin::Cw)]);
    ferry.resize(build.len(), Wait);
    let mut sim = Sim::empty();
    sim.glyphs.push(Some(Glyph {
        kind: GlyphKind::Source,
        at: Hex::new(1, 0),
        dir: 0,
    }));
    sim.glyphs.push(Some(Glyph {
        kind: GlyphKind::Output,
        at: Hex::new(0, 1),
        dir: 3,
    }));
    sim.glyphs.push(Some(Glyph {
        kind: GlyphKind::Bonder,
        at: Hex::new(0, -1),
        dir: 0,
    }));
    sim.glyphs.push(Some(Glyph {
        kind: GlyphKind::SecondBond,
        at: Hex::new(-1, -1),
        dir: 5,
    }));
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
        sim.glyphs = glyphs.into_iter().map(Some).collect();

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
        let mut sim = bench(vec![Grab, Rot(Spin::Cw)], Vec::new());
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
    fn a_grab_over_an_empty_cell_stalls() {
        let mut sim = bench(vec![Instr::Grab], Vec::new());
        sim.step();
        assert_eq!(sim.arms[0].stall, Some(Stall::Illegal));
        assert_eq!(sim.arms[0].pc, 0);
        assert!(!sim.arms[0].holding);
    }
    #[test]
    fn a_rotation_into_an_occupied_cell_stalls_until_it_clears() {
        use Instr::*;
        let mut sim = bench(vec![Grab, Rot(Spin::Cw)], Vec::new());
        sim.arms.push(Arm::new(
            Hex::new(2, -2),
            4,
            vec![Wait, Wait, Grab, Rot(Spin::Cw)],
        ));
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
    fn a_grab_of_a_held_atom_puts_a_second_hand_on_it_and_rotates_stall_until_a_drop() {
        use Instr::*;
        let mut sim = bench(vec![Wait, Grab, Rot(Spin::Cw)], Vec::new());
        sim.arms
            .push(Arm::new(Hex::new(2, 0), 3, vec![Grab, Wait, Drop, Wait]));
        put(&mut sim, 1, 0);
        sim.step();
        sim.step();
        assert_eq!(sim.arms[0].stall, None);
        assert_eq!(sim.held(0), Some(0));
        assert_eq!(sim.held(1), Some(0));
        sim.step();
        assert_eq!(sim.arms[0].stall, Some(Stall::Hand(1)));
        assert_eq!(sim.held(1), None);
        sim.step();
        assert_eq!(sim.arms[0].stall, None);
        assert_eq!(sim.atoms[0].unwrap().pos, Hex::new(1, -1));
    }

    #[test]
    fn two_arms_contending_for_one_cell_resolve_in_arm_order() {
        use Instr::*;
        let mut sim = bench(vec![Grab, Rot(Spin::Cw), Rot(Spin::Cw), Wait], Vec::new());
        sim.arms
            .push(Arm::new(Hex::new(2, -2), 3, vec![Grab, Rot(Spin::Cw)]));
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
    fn every_rule_names_only_its_own_slots_and_no_two_slots_share_a_cell() {
        for kind in GlyphKind::ALL {
            let rule = kind.rule();
            let n = rule.slots.len();
            assert!(n > 0, "{kind:?}");
            for (i, slot) in rule.slots.iter().enumerate() {
                assert!(
                    !rule.slots[..i].iter().any(|s| s.at == slot.at),
                    "{kind:?} slot {i} shares a cell"
                );
            }
            for (a, b, _) in rule.before {
                assert!(a != b && *a < n && *b < n, "{kind:?} before {a} {b}");
            }
            for (a, b, _) in rule.after {
                assert!(a != b && *a < n && *b < n, "{kind:?} after {a} {b}");
            }
        }
    }

    fn chain(sim: &mut Sim, q: i32, len: i32) {
        let mut prev = put(sim, q, 0);
        for r in 1..len {
            let next = put(sim, q, r);
            bond(sim, prev, next, BondKind::Single);
            prev = next;
        }
    }

    fn bonder_joining_chains(left: i32, right: i32) -> Sim {
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(0, 0),
            dir: 0,
        };
        let mut sim = bench(vec![Instr::Wait], vec![bonder]);
        chain(&mut sim, 0, left);
        chain(&mut sim, 1, right);
        sim
    }

    #[test]
    fn a_bond_that_would_pass_the_compound_cap_is_refused_and_one_that_meets_it_fires() {
        let left = MAX_COMPOUND_ATOMS as i32 - 2;
        let mut sim = bonder_joining_chains(left, 3);
        let before = sim.clone();
        sim.step();
        assert_eq!(sim.atoms, before.atoms);
        assert_eq!(sim.bonds, before.bonds);

        let mut sim = bonder_joining_chains(left, 2);
        sim.step();
        assert_eq!(sim.bonds.len(), MAX_COMPOUND_ATOMS - 1);
        assert_eq!(sim.component(0).len(), MAX_COMPOUND_ATOMS);
    }

    #[test]
    fn a_glyph_turns_with_its_dir_and_the_bonder_joins_two_loose_atoms_and_eats_nothing() {
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(1, 0),
            dir: 1,
        };
        assert_eq!(
            bonder.slots().collect::<Vec<_>>(),
            [Hex::new(1, 0), Hex::new(2, -1)]
        );
        let mut sim = bench(vec![Instr::Wait], vec![bonder]);
        let a = put(&mut sim, 1, 0);
        let b = put(&mut sim, 2, -1);
        sim.step();
        assert_eq!(sim.atoms.iter().flatten().count(), 2);
        assert_eq!(
            sim.bonds,
            vec![Bond {
                a,
                b,
                kind: BondKind::Single
            }]
        );
        sim.bonds[0].kind = BondKind::Double;
        sim.step();
        assert_eq!(sim.atoms.iter().flatten().count(), 2);
        assert_eq!(sim.bonds.len(), 1);
        assert_eq!(sim.bonds[0].kind, BondKind::Double);
    }

    #[test]
    fn the_second_bond_fires_only_with_its_roles_in_place() {
        let second = Glyph {
            kind: GlyphKind::SecondBond,
            at: Hex::new(1, 0),
            dir: 1,
        };
        assert_eq!(
            second.slots().collect::<Vec<_>>(),
            [Hex::new(1, 0), Hex::new(2, -1), Hex::new(1, -1)]
        );
        let single = |sim: &Sim| {
            sim.bonds
                .iter()
                .filter(|b| b.kind == BondKind::Single)
                .count()
        };
        let mut sim = bench(vec![Instr::Wait], vec![second]);
        let sacrificial = put(&mut sim, 1, 0);
        let a = put(&mut sim, 2, -1);
        let b = put(&mut sim, 1, -1);
        sim.step();
        assert_eq!(sim.atoms.iter().flatten().count(), 3);
        assert!(sim.bonds.is_empty());
        bond(&mut sim, sacrificial, a, BondKind::Single);
        sim.step();
        assert_eq!(sim.atoms.iter().flatten().count(), 3);
        assert_eq!(single(&sim), 1);
        bond(&mut sim, b, a, BondKind::Single);
        sim.step();
        assert_eq!(sim.atoms.iter().flatten().count(), 3);
        assert_eq!(single(&sim), 2);
        sim.bonds
            .retain(|x| x.a != sacrificial && x.b != sacrificial);
        sim.step();
        assert_eq!(sim.atoms[sacrificial], None);
        assert_eq!(
            sim.bonds,
            vec![Bond {
                a: b,
                b: a,
                kind: BondKind::Double
            }]
        );
        sim.step();
        assert_eq!(sim.bonds.len(), 1);
    }

    #[test]
    fn a_second_bond_leaves_a_bonded_sacrificial_atom_and_everything_else_untouched() {
        let second = Glyph {
            kind: GlyphKind::SecondBond,
            at: Hex::new(1, 0),
            dir: 1,
        };
        let mut sim = bench(vec![Instr::Wait], vec![second]);
        let sacrificial = put(&mut sim, 1, 0);
        let a = put(&mut sim, 2, -1);
        let b = put(&mut sim, 1, -1);
        let tail = put(&mut sim, 2, 0);
        bond(&mut sim, sacrificial, tail, BondKind::Single);
        bond(&mut sim, a, b, BondKind::Single);
        let before = sim.clone();
        sim.step();
        assert_eq!(sim.atoms, before.atoms);
        assert_eq!(sim.bonds, before.bonds);
    }

    #[test]
    fn a_second_bond_eats_a_lone_sacrificial_atom_and_doubles_the_bond() {
        let second = Glyph {
            kind: GlyphKind::SecondBond,
            at: Hex::new(1, 0),
            dir: 1,
        };
        let mut sim = bench(vec![Instr::Wait], vec![second]);
        let sacrificial = put(&mut sim, 1, 0);
        let a = put(&mut sim, 2, -1);
        let b = put(&mut sim, 1, -1);
        bond(&mut sim, a, b, BondKind::Single);
        sim.step();
        assert_eq!(sim.atoms[sacrificial], None);
        assert_eq!(
            sim.bonds,
            vec![Bond {
                a,
                b,
                kind: BondKind::Double
            }]
        );
    }

    #[test]
    fn machines_never_collide_so_a_bonder_laid_across_a_second_bond_feeds_it() {
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(2, -1),
            dir: 3,
        };
        let second = Glyph {
            kind: GlyphKind::SecondBond,
            at: Hex::new(1, 0),
            dir: 1,
        };
        assert_eq!(
            bonder.slots().collect::<Vec<_>>(),
            second.slots().skip(1).collect::<Vec<_>>()
        );
        let mut sim = bench(vec![Instr::Wait], vec![bonder, second]);
        put(&mut sim, 1, 0);
        put(&mut sim, 2, -1);
        put(&mut sim, 1, -1);
        sim.step();
        assert_eq!(sim.bonds.len(), 1);
        assert_eq!(sim.bonds[0].kind, BondKind::Double);
        assert_eq!(sim.atoms.iter().flatten().count(), 2);
    }

    #[test]
    fn an_output_takes_only_the_exact_shape_atoms_and_bonds_out_of_every_hand() {
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
        sim.arms[0].tape = vec![Instr::Grab, Instr::Wait];
        sim.arms[0].pc = 0;
        sim.step();
        assert_eq!(sim.delivered, 1);
        assert!(sim.arms[0].holding);
        assert_eq!(sim.held(0), None);
        assert!(sim.atoms.iter().flatten().next().is_none());
        assert!(sim.bonds.is_empty());
    }

    #[test]
    fn a_glyph_eats_a_held_atom_and_the_hand_stays_closed_on_whatever_comes_next() {
        use Instr::*;
        let second = Glyph {
            kind: GlyphKind::SecondBond,
            at: Hex::new(1, 0),
            dir: 1,
        };
        let mut sim = bench(vec![Grab, Wait], vec![second]);
        let sacrificial = put(&mut sim, 1, 0);
        let a = put(&mut sim, 2, -1);
        let b = put(&mut sim, 1, -1);
        bond(&mut sim, a, b, BondKind::Single);
        sim.step();
        assert!(sim.arms[0].holding);
        assert_eq!(sim.held(0), None);
        assert_eq!(sim.atoms[sacrificial], None);
        assert_eq!(sim.bonds.len(), 1);
        let arrived = put(&mut sim, 1, 0);
        sim.step();
        assert_eq!(sim.held(0), Some(arrived));
    }

    #[test]
    fn a_second_bond_fires_on_atoms_an_arm_holds_and_the_arm_keeps_the_compound() {
        use Instr::*;
        let second = Glyph {
            kind: GlyphKind::SecondBond,
            at: Hex::new(1, -1),
            dir: 0,
        };
        assert_eq!(
            second.slots().collect::<Vec<_>>(),
            [Hex::new(1, -1), Hex::new(2, -1), Hex::new(2, -2)]
        );
        let mut sim = bench(vec![Grab, Rot(Spin::Cw)], vec![second]);
        sim.arms[0].pivot = Hex::new(1, 0);
        sim.arms[0].dir = 1;
        let sacrificial = put(&mut sim, 1, -1);
        let held = put(&mut sim, 2, -1);
        let other = put(&mut sim, 2, -2);
        bond(&mut sim, held, other, BondKind::Single);
        sim.step();
        assert_eq!(sim.held(0), Some(held));
        assert_eq!(sim.atoms[sacrificial], None);
        assert_eq!(
            sim.bonds,
            vec![Bond {
                a: held,
                b: other,
                kind: BondKind::Double
            }]
        );
        sim.step();
        assert_eq!(sim.atoms[held].unwrap().pos, Hex::new(1, -1));
        assert_eq!(sim.atoms[other].unwrap().pos, Hex::new(0, -1));
    }

    #[test]
    fn a_drop_and_a_grab_of_one_atom_in_one_tick_end_the_same_in_either_arm_order() {
        use Instr::*;
        let dropper = Arm::new(Hex::new(0, 0), 0, vec![Grab, Drop, Wait]);
        let grabber = Arm::new(Hex::new(2, 0), 3, vec![Wait, Grab, Wait]);
        let ends: Vec<_> = [true, false]
            .into_iter()
            .map(|dropper_first| {
                let mut sim = Sim::empty();
                sim.arms = if dropper_first {
                    vec![dropper.clone(), grabber.clone()]
                } else {
                    vec![grabber.clone(), dropper.clone()]
                };
                put(&mut sim, 1, 0);
                sim.step();
                sim.step();
                let (d, g) = if dropper_first { (0, 1) } else { (1, 0) };
                (sim.arms[d].clone(), sim.arms[g].clone())
            })
            .collect();
        assert_eq!(ends[0], ends[1]);
        let (d, g) = &ends[0];
        assert!(!d.holding);
        assert!(g.holding);
        assert_eq!(g.stall, None);
    }

    fn held_pair(tape: Vec<Instr>) -> (Sim, usize, usize) {
        let mut sim = bench(tape, Vec::new());
        let a = put(&mut sim, 1, 0);
        let b = put(&mut sim, 2, -1);
        bond(&mut sim, a, b, BondKind::Single);
        (sim, a, b)
    }

    #[test]
    fn a_pivot_turns_the_held_pair_about_the_hand_and_leaves_the_arm_still() {
        use Instr::*;
        let (mut sim, a, b) = held_pair(vec![Grab, Pivot(Spin::Cw), Pivot(Spin::Ccw)]);
        sim.step();
        sim.step();
        assert_eq!(sim.arms[0].stall, None);
        assert_eq!(sim.arms[0].dir, 0);
        assert_eq!(sim.atoms[a].unwrap().pos, Hex::new(1, 0));
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(1, -1));
        sim.step();
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(2, -1));
        assert_eq!(sim.arms[0].pc, 3);
    }

    #[test]
    fn a_pivot_with_an_open_hand_or_a_hand_closed_on_nothing_moves_nothing_and_advances_the_tape() {
        use Instr::*;
        let (mut sim, a, b) = held_pair(vec![Pivot(Spin::Cw)]);
        sim.step();
        assert_eq!(sim.arms[0].stall, None);
        assert_eq!(sim.arms[0].pc, 1);
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(2, -1));
        sim.atoms[a] = None;
        sim.arms[0].holding = true;
        sim.step();
        assert_eq!(sim.arms[0].stall, None);
        assert_eq!(sim.arms[0].pc, 2);
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(2, -1));
    }

    #[test]
    fn a_pivot_into_an_occupied_cell_stalls() {
        use Instr::*;
        let (mut sim, _, b) = held_pair(vec![Grab, Pivot(Spin::Cw)]);
        put(&mut sim, 1, -1);
        sim.step();
        sim.step();
        assert_eq!(sim.arms[0].stall, Some(Stall::Illegal));
        assert_eq!(sim.arms[0].pc, 1);
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(2, -1));
    }

    #[test]
    fn a_pivot_under_two_hands_stalls_and_names_the_other_hand() {
        use Instr::*;
        let (mut sim, _, b) = held_pair(vec![Grab, Pivot(Spin::Cw)]);
        sim.arms
            .push(Arm::new(Hex::new(3, -1), 3, vec![Grab, Wait, Drop, Wait]));
        sim.step();
        sim.step();
        assert_eq!(sim.arms[0].stall, Some(Stall::Hand(1)));
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(2, -1));
        sim.step();
        sim.step();
        assert_eq!(sim.arms[0].stall, None);
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(1, -1));
    }

    #[test]
    fn a_rotate_under_two_hands_stalls_and_names_the_other_hand_until_it_drops() {
        use Instr::*;
        let mut sim = bench(vec![Grab, Rot(Spin::Cw)], Vec::new());
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
    fn an_output_turned_away_from_the_compound_ignores_it() {
        let mut sim = bench(vec![Instr::Wait], Vec::new());
        let a = put(&mut sim, 1, 0);
        let b = put(&mut sim, 1, 1);
        bond(&mut sim, a, b, BondKind::Double);
        for dir in 0..6 {
            sim.glyphs = vec![Some(Glyph {
                kind: GlyphKind::Output,
                at: Hex::new(1, 0),
                dir,
            })];
            sim.step();
            assert_eq!(sim.delivered, u64::from(dir == 5), "dir {dir}");
        }
    }

    #[test]
    fn preloaded_world_delivers_every_period_except_the_copy_that_stalls() {
        let mut sim = preloaded();
        for _ in 0..20 * 4 {
            sim.step();
        }
        assert_eq!(sim.delivered, 4 * (PLACEMENTS.len() as u64 - 1));
        let stalled: Vec<usize> = (0..sim.arms.len())
            .filter(|i| sim.arms[*i].stall.is_some())
            .collect();
        assert_eq!(stalled, vec![sim.arms.len() - 2, sim.arms.len() - 1]);
        assert!(sim.atoms.len() < 40);
    }

    fn cleanup(at: Hex) -> Glyph {
        Glyph {
            kind: GlyphKind::Cleanup,
            at,
            dir: 0,
        }
    }

    #[test]
    fn a_cleanup_eats_the_middle_of_a_chain_and_leaves_the_ends_where_they_lay_unbonded() {
        let mut sim = Sim::empty();
        sim.glyphs.push(Some(cleanup(ORIGIN)));
        let left = put(&mut sim, -1, 0);
        let mid = put(&mut sim, 0, 0);
        let right = put(&mut sim, 1, 0);
        bond(&mut sim, left, mid, BondKind::Single);
        bond(&mut sim, mid, right, BondKind::Double);
        sim.step();
        assert!(sim.bonds.is_empty());
        assert_eq!(sim.atoms[mid], None);
        assert_eq!(sim.atoms[left].unwrap().pos, Hex::new(-1, 0));
        assert_eq!(sim.atoms[right].unwrap().pos, Hex::new(1, 0));
    }

    #[test]
    fn a_cleanup_is_spent_by_its_first_meal_and_a_later_atom_on_its_cell_survives() {
        let mut sim = Sim::empty();
        let source = Some(Glyph {
            kind: GlyphKind::Source,
            at: Hex::new(-3, 0),
            dir: 0,
        });
        sim.glyphs.push(source);
        sim.glyphs.push(Some(cleanup(ORIGIN)));

        put(&mut sim, 0, 0);
        sim.step();
        assert_eq!(sim.glyphs, vec![source, None]);
        let later = put(&mut sim, 0, 0);
        sim.step();
        assert_eq!(sim.glyphs, vec![source, None]);

        assert_eq!(sim.atoms[later].unwrap().pos, ORIGIN);
        assert!(sim.atom_at(Hex::new(-3, 0)).is_some());
    }

    #[test]
    fn a_move_translates_the_pose_and_the_six_moves_compose_to_a_ring() {
        let start = (Hex::new(2, -1), 3);
        let (pivot, dir) = Instr::Move(0).posed(start.0, start.1);
        assert_eq!((pivot, dir), (Hex::new(3, -1), 3));
        let ring = (0..6).fold(start, |(p, d), k| Instr::Move(k).posed(p, d));
        assert_eq!(ring, start);
        for k in 0..6 {
            let there = Instr::Move(k).posed(start.0, start.1);
            assert_eq!(Instr::Move((k + 3) % 6).posed(there.0, there.1), start);
            assert_ne!(there, start);
        }
        assert_eq!(Instr::Rot(Spin::Cw).posed(start.0, start.1), (start.0, 4));
        for instr in [
            Instr::Grab,
            Instr::Drop,
            Instr::Wait,
            Instr::Pivot(Spin::Cw),
        ] {
            assert_eq!(instr.posed(start.0, start.1), start);
        }
    }

    #[test]
    fn a_move_carries_the_arm_and_its_held_compound_one_cell() {
        use Instr::*;
        let (mut sim, a, b) = held_pair(vec![Grab, Move(1), Move(4)]);
        sim.step();
        sim.step();
        assert_eq!(sim.arms[0].stall, None);
        assert_eq!(sim.arms[0].pivot, Hex::new(1, -1));
        assert_eq!(sim.arms[0].dir, 0);
        assert_eq!(sim.arms[0].hand(), Hex::new(2, -1));
        assert_eq!(sim.atoms[a].unwrap().pos, Hex::new(2, -1));
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(3, -2));
        assert_eq!(sim.held(0), Some(a));
        sim.step();
        assert_eq!(sim.arms[0].pivot, ORIGIN);
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(2, -1));
        assert_eq!(sim.arms[0].pc, 3);
    }

    #[test]
    fn a_move_whose_held_atom_would_land_on_another_atom_stalls_until_it_clears() {
        use Instr::*;
        let mut sim = bench(vec![Grab, Move(0)], Vec::new());
        sim.arms.push(Arm::new(
            Hex::new(3, 0),
            3,
            vec![Wait, Wait, Grab, Rot(Spin::Cw)],
        ));
        put(&mut sim, 1, 0);
        put(&mut sim, 2, 0);
        sim.step();
        sim.step();
        assert_eq!(sim.arms[0].stall, Some(Stall::Illegal));
        assert_eq!(sim.arms[0].pivot, ORIGIN);
        assert_eq!(sim.atoms[0].unwrap().pos, Hex::new(1, 0));
        sim.step();
        sim.step();
        assert_eq!(sim.arms[0].stall, Some(Stall::Illegal));
        assert_eq!(sim.atoms[1].unwrap().pos, Hex::new(2, 1));
        sim.step();
        assert_eq!(sim.arms[0].stall, None);
        assert_eq!(sim.arms[0].pivot, Hex::new(1, 0));
        assert_eq!(sim.atoms[0].unwrap().pos, Hex::new(2, 0));
    }

    #[test]
    fn a_move_under_two_hands_stalls_and_names_the_other_hand() {
        use Instr::*;
        let mut sim = bench(vec![Grab, Move(2)], Vec::new());
        sim.arms
            .push(Arm::new(Hex::new(2, 0), 3, vec![Grab, Wait, Drop, Wait]));
        put(&mut sim, 1, 0);
        sim.step();
        sim.step();
        assert_eq!(sim.arms[0].stall, Some(Stall::Hand(1)));
        assert_eq!(sim.arms[0].pivot, ORIGIN);
        sim.step();
        sim.step();
        assert_eq!(sim.arms[0].stall, None);
        assert_eq!(sim.arms[0].pivot, Hex::new(0, -1));
        assert_eq!(sim.atoms[0].unwrap().pos, Hex::new(1, -1));
    }

    fn base_move_onto(sim: &mut Sim) -> Option<Stall> {
        sim.arms[0].tape = vec![Instr::Move(3)];
        sim.step();
        sim.arms[0].stall
    }

    #[test]
    fn a_base_stalls_on_a_glyph_another_base_or_an_atom_and_walks_onto_a_free_cell() {
        let mut sim = bench(Vec::new(), vec![cleanup(Hex::new(-1, 0))]);
        assert_eq!(base_move_onto(&mut sim), Some(Stall::Illegal));
        assert_eq!(sim.arms[0].pivot, ORIGIN);

        let mut sim = bench(Vec::new(), Vec::new());
        sim.arms.push(Arm::new(Hex::new(-1, 0), 0, Vec::new()));
        assert_eq!(base_move_onto(&mut sim), Some(Stall::Illegal));
        sim.arms.swap(0, 1);
        sim.arms[1].tape = vec![Instr::Move(3)];
        sim.step();
        assert_eq!(sim.arms[1].stall, Some(Stall::Illegal));
        assert_eq!(sim.arms[1].pivot, ORIGIN);

        let mut sim = bench(Vec::new(), Vec::new());
        put(&mut sim, -1, 0);
        assert_eq!(base_move_onto(&mut sim), Some(Stall::Illegal));

        let mut sim = bench(vec![Instr::Move(2)], vec![cleanup(Hex::new(1, -1))]);
        sim.arms.push(Arm::new(Hex::new(-1, -1), 0, Vec::new()));
        put(&mut sim, 0, -2);
        sim.step();
        assert_eq!(sim.arms[0].stall, None);
        assert_eq!(sim.arms[0].pivot, Hex::new(0, -1));
    }

    #[test]
    fn a_sweep_that_would_put_a_held_atom_on_a_base_stalls() {
        use Instr::*;
        let (mut sim, _, b) = held_pair(vec![Grab, Pivot(Spin::Ccw), Rot(Spin::Cw)]);
        sim.arms[0].pivot = Hex::new(2, 0);
        sim.arms[0].dir = 3;
        sim.step();
        sim.step();
        assert_eq!(sim.arms[0].stall, Some(Stall::Illegal));
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(2, -1));
        sim.arms[0].tape.remove(1);
        sim.arms.push(Arm::new(Hex::new(1, 1), 0, Vec::new()));
        sim.step();
        assert_eq!(sim.arms[0].stall, Some(Stall::Illegal));
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(2, -1));
        sim.arms.pop();
        sim.step();
        assert_eq!(sim.arms[0].stall, None);
        assert_eq!(sim.atoms[b].unwrap().pos, Hex::new(1, 0));
    }

    #[test]
    fn a_closed_hand_that_moves_onto_an_atom_holds_it() {
        use Instr::*;
        let mut sim = bench(vec![Move(0), Move(0)], Vec::new());
        sim.arms[0].holding = true;
        put(&mut sim, 2, 0);
        sim.step();
        assert_eq!(sim.held(0), Some(0));
        sim.step();
        assert_eq!(sim.arms[0].pivot, Hex::new(2, 0));
        assert_eq!(sim.atoms[0].unwrap().pos, Hex::new(3, 0));
    }
}
