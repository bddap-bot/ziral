use crate::form::{CONVERTER_INPUTS, Form, RECIPES, recipes};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

    pub const fn ring(self) -> i32 {
        let (q, r, s) = (self.q.abs(), self.r.abs(), (self.q + self.r).abs());
        if q >= r && q >= s {
            q
        } else if r >= s {
            r
        } else {
            s
        }
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Stall {
    Illegal,
    Hand(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickEvents {
    pub tick: u64,
    pub events: Vec<TickEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TickEvent {
    Fired {
        glyph: usize,
        machine: Machine,
        at: Hex,
    },
    BondWritten {
        glyph: usize,
        a: usize,
        b: usize,
        kind: BondKind,
    },
    Consumed {
        glyph: usize,
        atoms: Vec<usize>,
    },
    Spawned {
        glyph: usize,
        atom: usize,
        kind: AtomKind,
        at: Hex,
    },
    Grabbed {
        arm: usize,
        atom: usize,
        at: Hex,
    },
    Dropped {
        arm: usize,
        atom: usize,
        at: Hex,
    },
    Rotated {
        arm: usize,
        spin: Spin,
        at: Hex,
    },
    Pivoted {
        arm: usize,
        spin: Spin,
        at: Hex,
    },
    Moved {
        arm: usize,
        from: Hex,
        to: Hex,
    },
    Stalled {
        arm: usize,
        instruction: Instr,
        reason: Stall,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AtomKind {
    Base,
    Amber,
    Plum,
}

impl AtomKind {
    pub const ALL: [AtomKind; 3] = [AtomKind::Base, AtomKind::Amber, AtomKind::Plum];
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Atom {
    pub kind: AtomKind,
    pub pos: Hex,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum BondKind {
    Single,
    Double,
}

impl BondKind {
    pub const ALL: [BondKind; 2] = [BondKind::Single, BondKind::Double];
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Bond {
    pub a: usize,
    pub b: usize,
    pub kind: BondKind,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ActivationEnergy(u8);

impl ActivationEnergy {
    pub const FULL: Self = Self(3);

    pub const fn decayed(self) -> Self {
        Self(self.0.saturating_sub(1))
    }

    pub const fn level(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Arm {
    pub pivot: Hex,
    pub dir: usize,
    pub tape: Vec<Instr>,
    pub pc: usize,
    pub holding: bool,
    pub stall: Option<Stall>,
    pub energy: ActivationEnergy,
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
            energy: ActivationEnergy::default(),
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Tier {
    One,
    Two,
    Three,
}

impl Tier {
    pub const fn radius(self) -> i32 {
        match self {
            Tier::One => 1,
            Tier::Two => 2,
            Tier::Three => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum GlyphKind {
    Source,
    Bonder,
    SecondBond,
    Reification,
    Converter(AtomKind),
    Output(Tier),
}

impl GlyphKind {
    pub const ALL: [GlyphKind; 9] = [
        GlyphKind::Source,
        GlyphKind::Bonder,
        GlyphKind::SecondBond,
        GlyphKind::Reification,
        GlyphKind::Converter(AtomKind::Amber),
        GlyphKind::Converter(AtomKind::Plum),
        GlyphKind::Output(Tier::One),
        GlyphKind::Output(Tier::Two),
        GlyphKind::Output(Tier::Three),
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Machine {
    Arm,
    Glyph(GlyphKind),
}

impl Machine {
    pub const ALL: [Machine; GlyphKind::ALL.len() + 1] = {
        let mut all = [Machine::Arm; GlyphKind::ALL.len() + 1];
        let mut k = 0;
        while k < GlyphKind::ALL.len() {
            all[k + 1] = Machine::Glyph(GlyphKind::ALL[k]);
            k += 1;
        }
        all
    };

    pub fn recipe(self) -> Option<&'static Form> {
        Item::Machine(self).recipe()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Machine(Machine),
    Atom(AtomKind),
    Step,
    Token(Instr),
}

impl From<Machine> for Item {
    fn from(machine: Machine) -> Item {
        Item::Machine(machine)
    }
}

impl Item {
    fn index(self) -> Option<usize> {
        recipes()
            .iter()
            .position(|(item, _)| *item == self)
            .or_else(|| match self {
                Item::Atom(kind) => AtomKind::ALL
                    .iter()
                    .position(|other| *other == kind)
                    .map(|i| RECIPES.len() + i),
                _ => None,
            })
    }

    pub fn recipe(self) -> Option<&'static Form> {
        recipes()
            .iter()
            .find(|(item, _)| *item == self)
            .map(|(_, form)| form)
    }
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
const SOURCE: [Slot; 1] = [base(ORIGIN)];
const AMBER_CONVERTER: [Slot; 3] = [consumed(ORIGIN), consumed(DIRS[0]), consumed(DIRS[1])];
const PLUM_CONVERTER: [Slot; 3] = [
    Slot {
        kind: AtomKind::Amber,
        ..consumed(ORIGIN)
    },
    consumed(DIRS[0]),
    consumed(Hex::new(2, -1)),
];

const fn hexagon<const N: usize>(radius: i32) -> [Slot; N] {
    let mut cells = [consumed(ORIGIN); N];
    let mut n = 0;
    let mut q = -radius;
    while q <= radius {
        let mut r = -radius;
        while r <= radius {
            let cell = Hex::new(q, r);
            if cell.ring() <= radius {
                cells[n] = consumed(cell);
                n += 1;
            }
            r += 1;
        }
        q += 1;
    }
    assert!(n == N);
    cells
}

const OUTPUT_1: [Slot; 7] = hexagon(Tier::One.radius());
const OUTPUT_2: [Slot; 19] = hexagon(Tier::Two.radius());
const OUTPUT_3: [Slot; 37] = hexagon(Tier::Three.radius());
const REIFICATION: [Slot; 19] = hexagon(Tier::Two.radius());

const fn plain(slots: &'static [Slot]) -> Rule {
    Rule {
        slots,
        before: &[],
        after: &[],
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
            GlyphKind::Reification => plain(&REIFICATION),
            GlyphKind::Converter(AtomKind::Amber) => Rule {
                before: &[
                    (0, 1, Some(BondKind::Single)),
                    (0, 2, Some(BondKind::Single)),
                ],
                ..plain(&AMBER_CONVERTER)
            },
            GlyphKind::Converter(AtomKind::Plum) => Rule {
                before: &[
                    (0, 1, Some(BondKind::Single)),
                    (1, 2, Some(BondKind::Double)),
                ],
                ..plain(&PLUM_CONVERTER)
            },
            GlyphKind::Converter(AtomKind::Base) => panic!("the base atom has a source"),
            GlyphKind::Output(Tier::One) => plain(&OUTPUT_1),
            GlyphKind::Output(Tier::Two) => plain(&OUTPUT_2),
            GlyphKind::Output(Tier::Three) => plain(&OUTPUT_3),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Glyph {
    pub kind: GlyphKind,
    pub at: Hex,
    pub dir: usize,
    pub energy: ActivationEnergy,
}

impl Glyph {
    pub fn new(kind: GlyphKind, at: Hex, dir: usize) -> Self {
        Glyph {
            kind,
            at,
            dir,
            energy: ActivationEnergy::default(),
        }
    }

    pub fn slots(&self) -> impl Iterator<Item = Hex> + '_ {
        self.kind
            .rule()
            .slots
            .iter()
            .map(move |s| self.at.add(s.at.turned(self.dir)))
    }
}

pub const MAX_COMPOUND_ATOMS: usize = 256;

pub const DEFAULT_CAP: u32 = 16;
pub const MAX_CAP: u32 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Short {
    pub item: Item,
    pub have: u32,
    pub need: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Inventory {
    count: [u32; RECIPES.len() + AtomKind::ALL.len()],
    cap: [u32; RECIPES.len() + AtomKind::ALL.len()],
}

impl Inventory {
    pub const EMPTY: Inventory = Inventory {
        count: [0; RECIPES.len() + AtomKind::ALL.len()],
        cap: [DEFAULT_CAP; RECIPES.len() + AtomKind::ALL.len()],
    };

    pub fn count(&self, item: Item) -> Option<u32> {
        item.index().map(|i| self.count[i])
    }

    pub fn cap(&self, item: Item) -> Option<u32> {
        item.index().map(|i| self.cap[i])
    }

    pub fn full(&self, item: Item) -> bool {
        item.index().is_none_or(|i| self.count[i] >= self.cap[i])
    }

    pub fn valid(&self) -> bool {
        self.cap.iter().all(|cap| *cap <= MAX_CAP)
    }

    pub fn add(&mut self, item: Item) {
        if let Some(i) = item.index() {
            self.count[i] = self.count[i].saturating_add(1);
        }
    }

    #[must_use]
    pub fn spend(&mut self, item: Item) -> bool {
        self.spend_all(&[item]).is_ok()
    }

    pub fn spend_all(&mut self, bill: &[Item]) -> Result<(), Vec<Short>> {
        let mut need = [0; RECIPES.len() + AtomKind::ALL.len()];
        for item in bill {
            let i = item
                .index()
                .expect("a source is world-placed and never held");
            need[i] += 1;
        }
        let items = recipes()
            .iter()
            .map(|(item, _)| *item)
            .chain(AtomKind::ALL.map(Item::Atom));
        let short: Vec<Short> = items
            .enumerate()
            .filter(|(i, _)| need[*i] > self.count[*i])
            .map(|(i, item)| Short {
                item,
                have: self.count[i],
                need: need[i],
            })
            .collect();
        if !short.is_empty() {
            return Err(short);
        }
        for (count, need) in self.count.iter_mut().zip(need) {
            *count -= need;
        }
        Ok(())
    }

    pub fn set_cap(&mut self, item: Item, notches: i32) {
        if let Some(i) = item.index() {
            self.cap[i] = self.cap[i].saturating_add_signed(notches).min(MAX_CAP);
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Sim {
    pub glyphs: Vec<Option<Glyph>>,
    pub arms: Vec<Arm>,
    pub atoms: Vec<Option<Atom>>,
    pub bonds: Vec<Bond>,
    pub tick: u64,
    pub inventory: Inventory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Id {
    Arm(usize),
    Glyph(usize),
    Atom(usize),
}

impl Id {
    pub fn moves(self) -> bool {
        !matches!(self, Id::Glyph(_))
    }

    pub fn may_share(self, other: Id) -> bool {
        matches!(
            (self, other),
            (Id::Atom(_), Id::Glyph(_)) | (Id::Glyph(_), Id::Atom(_))
        )
    }
}

impl Sim {
    pub fn empty() -> Self {
        Sim {
            glyphs: Vec::new(),
            arms: Vec::new(),
            atoms: Vec::new(),
            bonds: Vec::new(),
            tick: 0,
            inventory: Inventory::EMPTY,
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

    pub fn consume(&mut self, ids: &[usize]) {
        let gone = |id: usize| ids.contains(&id);
        self.bonds.retain(|x| !gone(x.a) && !gone(x.b));
        for id in ids {
            self.atoms[*id] = None;
        }
    }

    pub fn fragment(&self, ids: &[usize], at: Hex) -> Sim {
        let mut sim = Sim::empty();
        for id in ids {
            let atom = self.atoms[*id].unwrap();
            sim.spawn(Atom {
                pos: atom.pos.sub(at),
                ..atom
            });
        }
        let index = |id: usize| ids.iter().position(|x| *x == id);
        sim.bonds.extend(self.bonds.iter().filter_map(|bond| {
            Some(Bond {
                a: index(bond.a)?,
                b: index(bond.b)?,
                ..*bond
            })
        }));
        sim
    }

    pub fn ids(&self) -> impl Iterator<Item = Id> + '_ {
        let arms = (0..self.arms.len()).map(Id::Arm);
        let glyphs = self.glyphs.iter().enumerate();
        let atoms = self.atoms.iter().enumerate();
        arms.chain(glyphs.filter_map(|(i, g)| g.map(|_| Id::Glyph(i))))
            .chain(atoms.filter_map(|(i, a)| a.map(|_| Id::Atom(i))))
    }

    pub fn stands(&self, id: Id) -> impl Iterator<Item = Hex> + '_ {
        let (arm, glyph, atom) = match id {
            Id::Arm(i) => (Some(self.arms[i].pivot), None, None),
            Id::Glyph(i) => (None, self.glyphs[i].as_ref(), None),
            Id::Atom(i) => (None, None, self.atoms[i].map(|a| a.pos)),
        };
        arm.into_iter()
            .chain(glyph.into_iter().flat_map(Glyph::slots))
            .chain(atom)
    }

    pub fn on(&self, cell: Hex) -> impl Iterator<Item = Id> + '_ {
        self.ids()
            .filter(move |id| self.stands(*id).any(|c| c == cell))
    }

    pub fn blocked<'a>(
        &'a self,
        set: &'a Sim,
        at: Hex,
        picked: &'a [Id],
    ) -> impl Iterator<Item = Id> + 'a {
        set.ids()
            .flat_map(move |id| set.stands(id).map(move |cell| (id, cell)))
            .flat_map(move |(id, cell)| {
                self.on(cell.add(at))
                    .filter(move |other| !id.may_share(*other))
            })
            .filter(move |other| !picked.contains(other))
    }

    pub fn fits(&self, set: &Sim, at: Hex, picked: &[Id]) -> bool {
        self.blocked(set, at, picked).next().is_none()
    }

    pub fn replay(&self, ticks: u64) -> Sim {
        self.replayed(ticks).0
    }

    pub fn replayed(&self, ticks: u64) -> (Sim, Vec<TickEvents>) {
        let mut sim = self.clone();
        let mut events = Vec::new();
        for _ in 0..ticks {
            events.push(sim.step());
        }
        (sim, events)
    }

    pub fn step(&mut self) -> TickEvents {
        for arm in &mut self.arms {
            arm.energy = arm.energy.decayed();
        }
        for glyph in self.glyphs.iter_mut().flatten() {
            glyph.energy = glyph.energy.decayed();
        }
        let mut events = Vec::new();
        for i in 0..self.glyphs.len() {
            let Some(g) = self.glyphs[i] else { continue };
            if g.kind == GlyphKind::Source && self.atom_at(g.at).is_none() {
                events.push(TickEvent::Fired {
                    glyph: i,
                    machine: Machine::Glyph(g.kind),
                    at: g.at,
                });
                let atom = self.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: g.at,
                });
                events.push(TickEvent::Spawned {
                    glyph: i,
                    atom,
                    kind: AtomKind::Base,
                    at: g.at,
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
            if self.act(i, instr, &mut events) {
                self.arms[i].pc = self.arms[i].pc.wrapping_add(1);
            }
        }
        for i in 0..self.glyphs.len() {
            let Some(g) = self.glyphs[i] else { continue };
            match g.kind {
                GlyphKind::Output(tier) => self.craft(i, g.at, tier, &mut events),
                GlyphKind::Source => {}
                _ => self.fire(i, g, &mut events),
            }
        }
        for event in &events {
            match event {
                TickEvent::Fired { glyph, .. } => {
                    self.glyphs[*glyph].as_mut().unwrap().energy = ActivationEnergy::FULL;
                }
                TickEvent::Rotated { arm, .. } => {
                    self.arms[*arm].energy = ActivationEnergy::FULL;
                }
                _ => {}
            }
        }
        let tick = self.tick;
        self.tick += 1;
        TickEvents { tick, events }
    }

    fn craft(&mut self, glyph: usize, centre: Hex, tier: Tier, events: &mut Vec<TickEvent>) {
        let within = |atom: Atom| atom.pos.sub(centre).ring() <= tier.radius();
        let mut seen = Vec::new();
        for id in 0..self.atoms.len() {
            let Some(atom) = self.atoms[id] else { continue };
            if !within(atom) || seen.contains(&id) {
                continue;
            }
            let compound = self.component(id);
            seen.extend(&compound);
            if !compound.iter().all(|c| within(self.atoms[*c].unwrap()))
                || !recipes()
                    .iter()
                    .any(|(_, r)| r.atoms().len() == compound.len())
            {
                continue;
            }
            let Some(item) = Form::of(&self.fragment(&compound, centre)).crafts() else {
                continue;
            };
            if self.inventory.full(item) {
                continue;
            }
            events.push(TickEvent::Fired {
                glyph,
                machine: Machine::Glyph(self.glyphs[glyph].unwrap().kind),
                at: centre,
            });
            events.push(TickEvent::Consumed {
                glyph,
                atoms: compound.clone(),
            });
            self.consume(&compound);
            self.inventory.add(item);
        }
    }

    fn matched(&self, g: Glyph) -> Option<Vec<usize>> {
        let rule = g.kind.rule();
        let ids: Vec<usize> = g
            .slots()
            .zip(rule.slots)
            .map(|(at, slot)| {
                self.atom_at(at)
                    .filter(|id| {
                        g.kind == GlyphKind::Reification && slot.at == ORIGIN
                            || self.atoms[*id].unwrap().kind == slot.kind
                    })
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
        if g.kind == GlyphKind::Reification {
            for a in 0..ids.len() {
                for b in a + 1..ids.len() {
                    let (aa, bb) = (self.atoms[ids[a]].unwrap(), self.atoms[ids[b]].unwrap());
                    if aa.pos.sub(bb.pos).ring() == 1 && self.bond_between(ids[a], ids[b]).is_none()
                    {
                        return None;
                    }
                }
            }
        }
        if let GlyphKind::Converter(kind) = g.kind {
            let text = CONVERTER_INPUTS
                .iter()
                .find(|(made, _)| *made == kind)
                .expect("a converter input")
                .1;
            let expected: Form = text.parse().expect("a converter input");
            let compound = self.component(ids[0]);
            if compound.len() != ids.len()
                || !ids.iter().all(|id| compound.contains(id))
                || Form::of(&self.fragment(&compound, g.at)) != expected
            {
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

    fn fire(&mut self, glyph: usize, g: Glyph, events: &mut Vec<TickEvent>) {
        let rule = g.kind.rule();
        let Some(ids) = self.matched(g) else {
            return;
        };
        if g.kind == GlyphKind::Reification {
            let centre = rule
                .slots
                .iter()
                .position(|slot| slot.at == ORIGIN)
                .unwrap();
            let item = Item::Atom(self.atoms[ids[centre]].unwrap().kind);
            if self.inventory.full(item) {
                return;
            }
        }
        events.push(TickEvent::Fired {
            glyph,
            machine: Machine::Glyph(g.kind),
            at: g.at,
        });
        for (a, b, kind) in rule.after {
            let (a, b) = (ids[*a], ids[*b]);
            match self.bond_between(a, b) {
                Some(i) => self.bonds[i].kind = *kind,
                None => self.bonds.push(Bond { a, b, kind: *kind }),
            }
            events.push(TickEvent::BondWritten {
                glyph,
                a,
                b,
                kind: *kind,
            });
        }
        let consumed: Vec<usize> = rule
            .slots
            .iter()
            .zip(&ids)
            .filter(|(slot, _)| slot.consumed)
            .map(|(_, id)| *id)
            .collect();
        let made = (g.kind == GlyphKind::Reification).then(|| {
            let centre = rule
                .slots
                .iter()
                .position(|slot| slot.at == ORIGIN)
                .unwrap();
            self.atoms[ids[centre]].unwrap().kind
        });
        if !consumed.is_empty() {
            events.push(TickEvent::Consumed {
                glyph,
                atoms: consumed.clone(),
            });
        }
        self.consume(&consumed);
        if let Some(kind) = made {
            self.inventory.add(Item::Atom(kind));
        }
        if let GlyphKind::Converter(kind) = g.kind {
            let atom = self.spawn(Atom { kind, pos: g.at });
            events.push(TickEvent::Spawned {
                glyph,
                atom,
                kind,
                at: g.at,
            });
        }
    }

    fn act(&mut self, i: usize, instr: Instr, events: &mut Vec<TickEvent>) -> bool {
        let from = self.arms[i].pivot;
        let held = self.held(i);
        let stall = self.exec(i, instr).err();
        self.arms[i].stall = stall;
        match stall {
            Some(reason) => events.push(TickEvent::Stalled {
                arm: i,
                instruction: instr,
                reason,
            }),
            None => match instr {
                Instr::Grab => events.push(TickEvent::Grabbed {
                    arm: i,
                    atom: self.held(i).expect("a successful grab holds an atom"),
                    at: self.arms[i].pivot,
                }),
                Instr::Drop => {
                    if let Some(atom) = held {
                        events.push(TickEvent::Dropped {
                            arm: i,
                            atom,
                            at: self.arms[i].pivot,
                        });
                    }
                }
                Instr::Rot(spin) => events.push(TickEvent::Rotated {
                    arm: i,
                    spin,
                    at: self.arms[i].pivot,
                }),
                Instr::Pivot(spin) => events.push(TickEvent::Pivoted {
                    arm: i,
                    spin,
                    at: self.arms[i].pivot,
                }),
                Instr::Move(_) => events.push(TickEvent::Moved {
                    arm: i,
                    from,
                    to: self.arms[i].pivot,
                }),
                Instr::Wait => {}
            },
        }
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
        let clear = |id: Id, at: Hex| {
            self.on(at).all(|other| {
                id.may_share(other)
                    || matches!(other, Id::Arm(j) if j == i)
                    || matches!(other, Id::Atom(a) if comp.contains(&a))
            })
        };
        let stepped = pivot != self.arms[i].pivot;
        if (stepped && !clear(Id::Arm(i), pivot))
            || moved
                .iter()
                .any(|(id, at)| !clear(Id::Atom(*id), *at) || *at == pivot)
        {
            return Err(Stall::Illegal);
        }
        for (id, at) in moved {
            self.atoms[id].as_mut().unwrap().pos = at;
        }
        Ok(())
    }

    pub fn bill(&self) -> Vec<Item> {
        let glyphs = self
            .glyphs
            .iter()
            .flatten()
            .map(|g| Item::Machine(Machine::Glyph(g.kind)));
        let arms = self.arms.iter().flat_map(|a| {
            std::iter::once(Item::Machine(Machine::Arm))
                .chain(a.tape.iter().map(|instr| Item::Token(*instr)))
        });
        let atoms = self
            .atoms
            .iter()
            .flatten()
            .map(|atom| Item::Atom(atom.kind));
        let double = self
            .bonds
            .iter()
            .filter(|bond| bond.kind == BondKind::Double)
            .map(|_| Item::Atom(AtomKind::Base));
        glyphs.chain(arms).chain(atoms).chain(double).collect()
    }

    pub fn place(&mut self, other: &Sim, at: Hex) -> Vec<usize> {
        let glyphs = other
            .glyphs
            .iter()
            .flatten()
            .map(|g| {
                let g = Glyph {
                    at: g.at.add(at),
                    ..*g
                };
                seat(&mut self.glyphs, g)
            })
            .collect();
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
        glyphs
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
    let cw = Rot(Spin::Cw);
    let ccw = Rot(Spin::Ccw);
    let out = Move(5);
    let back = Move(2);
    let mut build = vec![Grab, cw, cw, cw, Drop, ccw, ccw, ccw];
    build.extend([Grab, cw, cw, Drop, ccw, ccw]);
    build.extend([Grab, cw, cw, cw, out, out, Drop, back, back, ccw, ccw, ccw]);
    let mut ferry = vec![Wait; 5];
    ferry.extend([Grab, ccw, Drop, cw]);
    ferry.resize(build.len(), Wait);
    let mut sim = Sim::empty();
    sim.glyphs
        .push(Some(Glyph::new(GlyphKind::Source, Hex::new(1, 0), 0)));
    sim.glyphs.push(Some(Glyph::new(
        GlyphKind::Output(Tier::One),
        Hex::new(-2, 3),
        0,
    )));
    sim.glyphs
        .push(Some(Glyph::new(GlyphKind::Bonder, Hex::new(0, -1), 0)));
    sim.glyphs
        .push(Some(Glyph::new(GlyphKind::SecondBond, Hex::new(-2, 1), 0)));
    sim.arms.push(Arm::new(ORIGIN, 0, build));
    sim.arms.push(Arm::new(Hex::new(-2, 0), 0, ferry));
    sim
}

pub fn start() -> Sim {
    let mut sim = Sim::empty();
    sim.glyphs
        .push(Some(Glyph::new(GlyphKind::Source, Hex::new(-4, 1), 0)));
    sim.glyphs
        .push(Some(Glyph::new(GlyphKind::Bonder, Hex::new(-1, 1), 0)));
    sim.glyphs.push(Some(Glyph::new(
        GlyphKind::Output(Tier::One),
        Hex::new(2, -2),
        0,
    )));
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
        pos: Hex::new(1, -1).add(PLACEMENTS[PLACEMENTS.len() - 1]),
    });
    world
}

pub struct Fixture {
    pub sim: Sim,
    pub ticks: u64,
    pub done: fn(&Sim) -> bool,
}

fn armed(tape: Vec<Instr>) -> Sim {
    let mut sim = Sim::empty();
    sim.arms.push(Arm::new(ORIGIN, 0, tape));
    sim
}

fn crafted(s: &Sim) -> bool {
    s.arms.iter().all(|a| !a.holding)
        && s.atoms.iter().all(Option::is_none)
        && s.glyphs
            .iter()
            .flatten()
            .all(|g| s.inventory.count(Item::Machine(Machine::Glyph(g.kind))) == Some(1))
}

pub fn fixture(machine: Machine) -> Fixture {
    use Instr::{Drop, Grab, Rot};
    let (cw, ccw) = (Rot(Spin::Cw), Rot(Spin::Ccw));
    let base = |pos| Atom {
        kind: AtomKind::Base,
        pos,
    };
    match machine {
        Machine::Arm => {
            let mut sim = armed(vec![Grab, cw, Drop]);
            sim.spawn(base(DIRS[0]));
            Fixture {
                sim,
                ticks: 3,
                done: |s| s.atom_at(DIRS[1]).is_some() && !s.arms[0].holding,
            }
        }
        Machine::Glyph(GlyphKind::Source) => {
            let mut sim = Sim::empty();
            sim.glyphs
                .push(Some(Glyph::new(GlyphKind::Source, ORIGIN, 0)));
            Fixture {
                sim,
                ticks: 1,
                done: |s| s.atom_at(ORIGIN).is_some(),
            }
        }
        Machine::Glyph(GlyphKind::Bonder) => {
            let mut sim = armed(vec![Grab, cw, cw, Drop, ccw, ccw, Grab, cw, Drop]);
            sim.glyphs
                .push(Some(Glyph::new(GlyphKind::Source, DIRS[0], 0)));
            sim.glyphs
                .push(Some(Glyph::new(GlyphKind::Bonder, DIRS[2], 0)));
            Fixture {
                sim,
                ticks: 9,
                done: |s| {
                    let (Some(a), Some(b)) = (s.atom_at(DIRS[1]), s.atom_at(DIRS[2])) else {
                        return false;
                    };
                    !s.arms[0].holding
                        && s.bond_between(a, b)
                            .is_some_and(|i| s.bonds[i].kind == BondKind::Single)
                },
            }
        }
        Machine::Glyph(GlyphKind::SecondBond) => {
            let mut sim = armed(vec![Grab, cw, Drop]);
            sim.glyphs
                .push(Some(Glyph::new(GlyphKind::Source, DIRS[0], 0)));
            let glyph = Glyph::new(GlyphKind::SecondBond, DIRS[1], 1);
            let bonded: Vec<usize> = glyph
                .slots()
                .skip(1)
                .map(|at| sim.spawn(base(at)))
                .collect();
            sim.bonds.push(Bond {
                a: bonded[0],
                b: bonded[1],
                kind: BondKind::Single,
            });
            sim.glyphs.push(Some(glyph));
            Fixture {
                sim,
                ticks: 3,
                done: |s| {
                    !s.arms[0].holding
                        && s.atom_at(DIRS[1]).is_none()
                        && s.bonds.len() == 1
                        && s.bonds[0].kind == BondKind::Double
                },
            }
        }
        Machine::Glyph(GlyphKind::Reification) => {
            let form = machine
                .recipe()
                .expect("the reification glyph has a recipe");
            let centre = form.centre(Tier::Two.radius()).expect("a centred wrap");
            let mut sim = Sim::empty();
            sim.place(&form.sim(), ORIGIN.sub(centre));
            sim.glyphs
                .push(Some(Glyph::new(GlyphKind::Reification, ORIGIN, 0)));
            Fixture {
                sim,
                ticks: 1,
                done: |s| {
                    s.atoms.iter().all(Option::is_none)
                        && s.inventory.count(Item::Atom(AtomKind::Base)) == Some(1)
                },
            }
        }
        Machine::Glyph(GlyphKind::Converter(AtomKind::Amber)) => {
            let glyph = Glyph::new(GlyphKind::Converter(AtomKind::Amber), ORIGIN, 0);
            let mut sim = Sim::empty();
            let ids: Vec<usize> = glyph.slots().map(|pos| sim.spawn(base(pos))).collect();
            sim.bonds.push(Bond {
                a: ids[0],
                b: ids[1],
                kind: BondKind::Single,
            });
            sim.bonds.push(Bond {
                a: ids[0],
                b: ids[2],
                kind: BondKind::Single,
            });
            sim.glyphs.push(Some(glyph));
            Fixture {
                sim,
                ticks: 1,
                done: |s| {
                    s.atoms.iter().flatten().eq([&Atom {
                        kind: AtomKind::Amber,
                        pos: ORIGIN,
                    }])
                },
            }
        }
        Machine::Glyph(GlyphKind::Converter(AtomKind::Plum)) => {
            let glyph = Glyph::new(GlyphKind::Converter(AtomKind::Plum), ORIGIN, 0);
            let mut sim = Sim::empty();
            let slots: Vec<Hex> = glyph.slots().collect();
            let ids = [
                sim.spawn(Atom {
                    kind: AtomKind::Amber,
                    pos: slots[0],
                }),
                sim.spawn(base(slots[1])),
                sim.spawn(base(slots[2])),
            ];
            sim.bonds.push(Bond {
                a: ids[0],
                b: ids[1],
                kind: BondKind::Single,
            });
            sim.bonds.push(Bond {
                a: ids[1],
                b: ids[2],
                kind: BondKind::Double,
            });
            sim.glyphs.push(Some(glyph));
            Fixture {
                sim,
                ticks: 1,
                done: |s| {
                    s.atoms.iter().flatten().eq([&Atom {
                        kind: AtomKind::Plum,
                        pos: ORIGIN,
                    }])
                },
            }
        }
        Machine::Glyph(GlyphKind::Converter(AtomKind::Base)) => {
            panic!("the base atom has a source")
        }
        Machine::Glyph(GlyphKind::Output(tier)) => {
            let form = machine.recipe().expect("every output tier has a recipe");
            let centre = form
                .centre(1)
                .expect("every recipe lies within the first tier");
            let held = form
                .atoms()
                .iter()
                .map(|(at, _)| *at)
                .find(|at| at.sub(centre).ring() == 1)
                .expect("a recipe has an atom beside its centre");
            let turn = (0..6)
                .find(|t| centre.sub(held).turned(*t) == DIRS[1])
                .expect("six turns reach every direction");
            let landed = |at: Hex| DIRS[1].add(at.sub(held).turned(turn));
            let grabbed = |at: Hex| landed(at).rotate(ORIGIN, Spin::Ccw);
            let mut compound = form.sim();
            for atom in compound.atoms.iter_mut().flatten() {
                atom.pos = grabbed(atom.pos);
            }
            let reach = tier.radius() + 1;
            let mut sim = armed(vec![Grab, cw, Drop]);
            sim.place(&compound, ORIGIN);
            sim.glyphs.push(Some(Glyph::new(
                GlyphKind::Output(tier),
                Hex::new(DIRS[1].q * reach, DIRS[1].r * reach),
                0,
            )));
            Fixture {
                sim,
                ticks: 3,
                done: crafted,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_energy_is_set_by_its_tick_event_and_decays_exactly_on_the_tick_grid() {
        let mut sim = Sim::empty();
        sim.glyphs
            .push(Some(Glyph::new(GlyphKind::Source, ORIGIN, 0)));

        assert_eq!(sim.glyphs[0].unwrap().energy.level(), 0);
        sim.step();
        assert_eq!(sim.glyphs[0].unwrap().energy.level(), 3);
        sim.step();
        assert_eq!(sim.glyphs[0].unwrap().energy.level(), 2);
        sim.step();
        assert_eq!(sim.glyphs[0].unwrap().energy.level(), 1);
        sim.step();
        assert_eq!(sim.glyphs[0].unwrap().energy.level(), 0);
    }

    fn source(at: Hex) -> Glyph {
        Glyph {
            kind: GlyphKind::Source,
            at,
            dir: 0,
            energy: ActivationEnergy::default(),
        }
    }

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
            energy: ActivationEnergy::default(),
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
            energy: ActivationEnergy::default(),
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
            energy: ActivationEnergy::default(),
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
            energy: ActivationEnergy::default(),
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
            energy: ActivationEnergy::default(),
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

    fn output(tier: Tier, at: Hex) -> Glyph {
        Glyph {
            kind: GlyphKind::Output(tier),
            at,
            dir: 0,
            energy: ActivationEnergy::default(),
        }
    }

    fn lay(sim: &mut Sim, recipe: &Form, turn: usize, at: Hex) -> Vec<usize> {
        let mut set = recipe.sim();
        for atom in set.atoms.iter_mut().flatten() {
            atom.pos = atom.pos.turned(turn);
        }
        let before: Vec<bool> = sim.atoms.iter().map(Option::is_some).collect();
        sim.place(&set, at);
        (0..sim.atoms.len())
            .filter(|i| sim.atoms[*i].is_some() && !before.get(*i).copied().unwrap_or(false))
            .collect()
    }

    fn count(sim: &Sim, item: impl Into<Item>) -> u32 {
        sim.inventory.count(item.into()).unwrap()
    }

    fn lying(sim: &Sim, ids: &[usize]) -> bool {
        ids.iter().all(|id| sim.atoms[*id].is_some())
    }

    #[test]
    fn every_recipe_is_one_distinct_compound_bonded_across_adjacent_cells_that_fits_its_tier() {
        let forms: Vec<&Form> = recipes().iter().map(|(_, form)| form).collect();
        for (k, (item, recipe)) in recipes().iter().enumerate() {
            let sim = recipe.sim();
            let tier = if matches!(
                *item,
                Item::Machine(Machine::Glyph(
                    GlyphKind::Reification | GlyphKind::Converter(AtomKind::Plum)
                ))
            ) {
                Tier::Two
            } else {
                Tier::One
            };
            let centre = recipe
                .centre(tier.radius())
                .unwrap_or_else(|| panic!("{item:?} hangs off its tier"));
            assert_eq!(
                sim.component(0).len(),
                recipe.atoms().len(),
                "{item:?} is not one compound"
            );
            for bond in &sim.bonds {
                let (a, b) = (
                    sim.atoms[bond.a].unwrap().pos,
                    sim.atoms[bond.b].unwrap().pos,
                );
                let kind = &bond.kind;
                assert_eq!(a.sub(b).ring(), 1, "{item:?} bonds cells that do not touch");
                if *kind == BondKind::Double {
                    let free = DIRS
                        .iter()
                        .map(|d| a.add(*d))
                        .filter(|c| c.sub(b).ring() == 1)
                        .any(|c| sim.atom_at(c).is_none());
                    assert!(free, "{item:?} has a double bond no second-bond can write");
                }
            }
            assert!(!forms[..k].contains(&recipe), "{item:?} shares a recipe");
            assert!(
                !RECIPES[..k].iter().any(|(other, _)| other == item),
                "{item:?} is listed twice"
            );
            let mut world = Sim::empty();
            world.glyphs.push(Some(output(tier, ORIGIN)));
            lay(&mut world, recipe, 0, ORIGIN.sub(centre));
            world.step();
            assert_eq!(count(&world, *item), 1, "{item:?} does not craft itself");
            assert!(world.atoms.iter().flatten().next().is_none());
        }
        assert!(
            Item::Machine(Machine::Glyph(GlyphKind::Source))
                .recipe()
                .is_none()
        );
    }

    #[test]
    fn converter_inputs_and_every_recipe_are_distinct_under_every_turn() {
        let mut forms: Vec<Form> = recipes().iter().map(|(_, form)| form.clone()).collect();
        forms.extend(
            CONVERTER_INPUTS
                .iter()
                .map(|(_, text)| text.parse().unwrap()),
        );
        for a in 0..forms.len() {
            for b in 0..a {
                for turn in 0..6 {
                    let mut turned = forms[a].sim();
                    for atom in turned.atoms.iter_mut().flatten() {
                        atom.pos = atom.pos.turned(turn);
                    }
                    assert_ne!(
                        Form::of(&turned),
                        forms[b],
                        "forms {a} and {b}, turn {turn}"
                    );
                }
            }
        }
    }

    #[test]
    fn converters_leave_a_wrong_bond_or_turn_untouched() {
        for kind in [AtomKind::Amber, AtomKind::Plum] {
            let f = fixture(Machine::Glyph(GlyphKind::Converter(kind)));
            let mut wrong = f.sim.clone();
            wrong.bonds[0].kind = BondKind::Double;
            let before = wrong.clone();
            wrong.step();
            assert_eq!(wrong.atoms, before.atoms, "{kind:?} wrong bond");
            assert_eq!(wrong.bonds, before.bonds, "{kind:?} wrong bond");

            let mut turned = f.sim;
            for atom in turned.atoms.iter_mut().flatten() {
                atom.pos = atom.pos.turned(1);
            }
            let before = turned.clone();
            turned.step();
            assert_eq!(turned.atoms, before.atoms, "{kind:?} wrong turn");
            assert_eq!(turned.bonds, before.bonds, "{kind:?} wrong turn");

            let mut attached = fixture(Machine::Glyph(GlyphKind::Converter(kind))).sim;
            let tail = attached.spawn(Atom {
                kind: AtomKind::Base,
                pos: Hex::new(-1, 0),
            });
            attached.bonds.push(Bond {
                a: 0,
                b: tail,
                kind: BondKind::Single,
            });
            let before = attached.clone();
            attached.step();
            assert_eq!(attached.atoms, before.atoms, "{kind:?} attached atom");
            assert_eq!(attached.bonds, before.bonds, "{kind:?} attached atom");
        }
    }

    fn mirrored(form: &Form) -> Form {
        let mut sim = form.sim();
        for atom in sim.atoms.iter_mut().flatten() {
            atom.pos = Hex::new(atom.pos.r, atom.pos.q);
        }
        Form::of(&sim)
    }

    fn last_bend(form: &Form) -> i32 {
        let sim = form.sim();
        let degree = |i: usize| sim.bonds.iter().filter(|b| b.a == i || b.b == i).count();
        let double = sim
            .bonds
            .iter()
            .find(|b| b.kind == BondKind::Double)
            .expect("a token carries the arm's double bond");
        let mut path = vec![
            [double.a, double.b]
                .into_iter()
                .find(|i| degree(*i) == 1)
                .expect("a chain from the arm"),
        ];
        while let Some(next) = sim.bonds.iter().find_map(|b| {
            let last = *path.last().unwrap();
            let other = if b.a == last {
                b.b
            } else if b.b == last {
                b.a
            } else {
                return None;
            };
            (!path.contains(&other)).then_some(other)
        }) {
            path.push(next);
        }
        let at = |i: usize| sim.atoms[i].unwrap().pos;
        let [a, b, c] = [
            path[path.len() - 3],
            path[path.len() - 2],
            path[path.len() - 1],
        ]
        .map(at);
        let (u, v) = (b.sub(a), c.sub(b));
        u.q * v.r - u.r * v.q
    }

    #[test]
    fn the_step_is_one_atom_and_every_token_is_the_arm_with_its_act_drawn_on() {
        assert_eq!(Item::Step.recipe().unwrap().atoms().len(), 1);
        let recipe = |instr: Instr| Item::Token(instr).recipe().unwrap();
        let tokens: Vec<Instr> = recipes()
            .iter()
            .filter_map(|(item, _)| match item {
                Item::Token(instr) => Some(*instr),
                Item::Machine(_) | Item::Atom(_) | Item::Step => None,
            })
            .collect();
        for instr in tokens {
            let sim = recipe(instr).sim();
            assert!(
                sim.bonds.iter().any(|b| b.kind == BondKind::Double),
                "{instr:?} has no arm in it"
            );
            let atoms = if matches!(instr, Instr::Move(_)) {
                4
            } else {
                3
            };
            assert_eq!(sim.atoms.len(), atoms, "{instr:?}");
        }
        for instr in [Instr::Grab, Instr::Drop, Instr::Wait] {
            assert_eq!(mirrored(recipe(instr)), *recipe(instr), "{instr:?}");
        }
        for (ccw, cw) in [
            (Instr::Rot(Spin::Ccw), Instr::Rot(Spin::Cw)),
            (Instr::Pivot(Spin::Ccw), Instr::Pivot(Spin::Cw)),
            (Instr::Move(1), Instr::Move(4)),
            (Instr::Move(2), Instr::Move(5)),
            (Instr::Move(3), Instr::Move(0)),
        ] {
            assert_eq!(mirrored(recipe(ccw)), *recipe(cw), "{ccw:?} {cw:?}");
            assert!(last_bend(recipe(ccw)) > 0, "{ccw:?} bends clockwise");
            assert!(last_bend(recipe(cw)) < 0, "{cw:?} bends counterclockwise");
        }
        assert_eq!(last_bend(recipe(Instr::Drop)), 0);
        assert_eq!(last_bend(recipe(Instr::Wait)), 0);
        let a_lone_atom_on_a_pad = |tier| {
            let mut sim = Sim::empty();
            sim.glyphs.push(Some(output(tier, ORIGIN)));
            sim.spawn(Atom {
                kind: AtomKind::Base,
                pos: ORIGIN,
            });
            sim.step();
            (count(&sim, Item::Step), sim.atoms.iter().flatten().count())
        };
        assert_eq!(a_lone_atom_on_a_pad(Tier::One), (1, 0));
        assert_eq!(a_lone_atom_on_a_pad(Tier::Three), (1, 0));
    }

    #[test]
    fn a_larger_glyph_takes_a_recipe_in_every_turn_wherever_it_lies_wholly_on_its_cells() {
        let (item, recipe) = &recipes()[0];
        let item = *item;
        assert_eq!(recipe.atoms().len(), 2);
        let radius = Tier::Two.radius();
        let (mut fired, mut refused) = (0, 0);
        for turn in 0..6 {
            for q in -radius - 1..=radius + 1 {
                for r in -radius - 1..=radius + 1 {
                    let at = Hex::new(q, r);
                    let mut sim = Sim::empty();
                    sim.glyphs.push(Some(output(Tier::Two, ORIGIN)));
                    let ids = lay(&mut sim, recipe, turn, at);
                    let inside = |id: &usize| sim.atoms[*id].unwrap().pos.ring() <= radius;
                    let (wholly, touching) = (ids.iter().all(inside), ids.iter().any(inside));
                    sim.step();
                    if wholly {
                        fired += 1;
                        assert_eq!(count(&sim, item), 1, "turn {turn} at {at:?}");
                        assert!(!lying(&sim, &ids));
                    } else {
                        refused += usize::from(touching);
                        assert_eq!(count(&sim, item), 0, "turn {turn} at {at:?}");
                        assert!(lying(&sim, &ids));
                        assert_eq!(sim.bonds.len(), 1);
                    }
                }
            }
        }
        assert_eq!((fired, refused), (6 * 14, 6 * 10));
    }

    fn wrapped(centre_kind: AtomKind) -> Sim {
        let machine = Machine::Glyph(GlyphKind::Reification);
        let form = machine.recipe().unwrap();
        let centre = form.centre(Tier::Two.radius()).unwrap();
        let mut sim = Sim::empty();
        sim.place(&form.sim(), ORIGIN.sub(centre));
        let centre_atom = sim.atom_at(ORIGIN).unwrap();
        sim.atoms[centre_atom].as_mut().unwrap().kind = centre_kind;
        sim.glyphs
            .push(Some(Glyph::new(GlyphKind::Reification, ORIGIN, 0)));
        sim
    }

    #[test]
    fn the_complete_wrap_reifies_its_centre_kind() {
        for kind in AtomKind::ALL {
            let mut sim = wrapped(kind);
            sim.step();
            assert!(sim.atoms.iter().all(Option::is_none));
            assert!(sim.bonds.is_empty());
            assert_eq!(sim.inventory.count(Item::Atom(kind)), Some(1));
        }
    }

    #[test]
    fn a_full_atom_count_holds_the_wrap() {
        let mut sim = wrapped(AtomKind::Base);
        for _ in 0..DEFAULT_CAP {
            sim.inventory.add(Item::Atom(AtomKind::Base));
        }
        let before = sim.clone();
        sim.step();
        assert_eq!(sim.atoms, before.atoms);
        assert_eq!(sim.bonds, before.bonds);
        assert_eq!(sim.inventory, before.inventory);
    }

    #[test]
    fn a_wrap_missing_one_bond_or_holding_a_wrong_outer_kind_stays_untouched() {
        let complete = wrapped(AtomKind::Base);
        let mut missing = complete.clone();
        missing.bonds.pop();
        let before = missing.clone();
        missing.step();
        assert_eq!(missing.atoms, before.atoms);
        assert_eq!(missing.bonds, before.bonds);
        assert_eq!(missing.inventory, before.inventory);

        let mut wrong = complete;
        let outer = wrong.atom_at(Hex::new(2, 0)).unwrap();
        wrong.atoms[outer].as_mut().unwrap().kind = AtomKind::Amber;
        let before = wrong.clone();
        wrong.step();
        assert_eq!(wrong.atoms, before.atoms);
        assert_eq!(wrong.bonds, before.bonds);
        assert_eq!(wrong.inventory, before.inventory);
    }

    #[test]
    fn a_turn_is_the_same_shape_and_a_mirror_image_is_another() {
        let mut propeller = Sim::empty();
        let centre = put(&mut propeller, 0, 0);
        for k in [0, 2, 4] {
            let spoke = propeller.spawn(Atom {
                kind: AtomKind::Base,
                pos: DIRS[k],
            });
            let tip = propeller.spawn(Atom {
                kind: AtomKind::Base,
                pos: DIRS[k].add(DIRS[(k + 1) % 6]),
            });
            bond(&mut propeller, centre, spoke, BondKind::Single);
            bond(&mut propeller, spoke, tip, BondKind::Single);
        }
        let form = Form::of(&propeller);
        let mut turned = propeller.clone();
        for atom in turned.atoms.iter_mut().flatten() {
            atom.pos = atom.pos.turned(1).add(Hex::new(4, -7));
        }
        assert_eq!(Form::of(&turned), form);
        let mut mirrored = propeller.clone();
        for atom in mirrored.atoms.iter_mut().flatten() {
            atom.pos = Hex::new(atom.pos.q, -atom.pos.q - atom.pos.r);
        }
        assert_ne!(Form::of(&mirrored), form);
    }

    #[test]
    fn a_pair_crafts_by_its_bond_and_an_extra_atom_or_bond_leaves_the_compound_untouched() {
        let mut sim = bench(
            vec![Instr::Grab, Instr::Wait],
            vec![output(Tier::One, Hex::new(2, 0))],
        );
        let a = put(&mut sim, 1, 0);
        let b = put(&mut sim, 2, 0);
        bond(&mut sim, a, b, BondKind::Double);
        sim.step();
        assert_eq!(count(&sim, Machine::Arm), 1);
        assert_eq!(count(&sim, Machine::Glyph(GlyphKind::Bonder)), 0);
        assert!(sim.arms[0].holding);
        assert_eq!(sim.held(0), None);
        assert!(sim.atoms.iter().flatten().next().is_none());
        let arc: Vec<usize> = [(3, 0), (3, -1), (2, -1), (1, 0)]
            .into_iter()
            .map(|(q, r)| put(&mut sim, q, r))
            .collect();
        for pair in arc.windows(2) {
            bond(&mut sim, pair[0], pair[1], BondKind::Single);
        }
        sim.step();
        assert!(lying(&sim, &arc));
        assert_eq!(sim.bonds.len(), 3);
        sim.consume(&arc);
        let triangle = [
            put(&mut sim, 2, 0),
            put(&mut sim, 3, 0),
            put(&mut sim, 3, -1),
        ];
        bond(&mut sim, triangle[0], triangle[1], BondKind::Single);
        bond(&mut sim, triangle[1], triangle[2], BondKind::Double);
        bond(&mut sim, triangle[2], triangle[0], BondKind::Double);
        sim.step();
        assert!(lying(&sim, &triangle));
        assert_eq!(count(&sim, Machine::Glyph(GlyphKind::SecondBond)), 0);
        sim.bonds[1].kind = BondKind::Single;
        sim.bonds[2].kind = BondKind::Single;
        sim.step();
        assert!(!lying(&sim, &triangle));
        assert_eq!(count(&sim, Machine::Glyph(GlyphKind::SecondBond)), 1);
        assert_eq!(count(&sim, Machine::Arm), 1);
    }

    #[test]
    fn the_cap_holds_a_compound_until_the_count_drops_and_a_return_passes_it() {
        let bonder = Machine::Glyph(GlyphKind::Bonder);
        let recipe = bonder.recipe().unwrap();
        let mut sim = Sim::empty();
        sim.glyphs.push(Some(output(Tier::One, ORIGIN)));
        for _ in 1..DEFAULT_CAP {
            sim.inventory.add(bonder.into());
        }
        lay(&mut sim, recipe, 0, ORIGIN);
        sim.step();
        assert_eq!(count(&sim, bonder), DEFAULT_CAP);
        let ids = lay(&mut sim, recipe, 1, ORIGIN);
        for _ in 0..3 {
            sim.step();
        }
        assert!(lying(&sim, &ids));
        assert_eq!(count(&sim, bonder), DEFAULT_CAP);
        assert!(sim.inventory.spend(bonder.into()));
        sim.step();
        assert!(!lying(&sim, &ids));
        assert_eq!(count(&sim, bonder), DEFAULT_CAP);
        sim.inventory.add(bonder.into());
        sim.inventory.add(bonder.into());
        let ids = lay(&mut sim, recipe, 2, ORIGIN);
        sim.step();
        assert!(lying(&sim, &ids));
        assert_eq!(count(&sim, bonder), DEFAULT_CAP + 2);
        sim.inventory.set_cap(bonder.into(), 3);
        sim.step();
        assert!(!lying(&sim, &ids));
        assert_eq!(count(&sim, bonder), DEFAULT_CAP + 3);
        assert!(sim.inventory.full(bonder.into()));
    }

    #[test]
    fn two_compounds_on_one_glyph_both_craft_in_one_tick() {
        let mut sim = Sim::empty();
        sim.glyphs.push(Some(output(Tier::Two, ORIGIN)));
        let bonder = Machine::Glyph(GlyphKind::Bonder);
        let pair = lay(&mut sim, bonder.recipe().unwrap(), 0, Hex::new(-2, 0));
        let arm = lay(
            &mut sim,
            Item::Machine(Machine::Arm).recipe().unwrap(),
            0,
            Hex::new(1, 0),
        );
        sim.step();
        assert!(!lying(&sim, &pair) && !lying(&sim, &arm));
        assert_eq!((count(&sim, bonder), count(&sim, Machine::Arm)), (1, 1));
    }

    #[test]
    fn a_glyph_eats_a_held_atom_and_the_hand_stays_closed_on_whatever_comes_next() {
        use Instr::*;
        let second = Glyph {
            kind: GlyphKind::SecondBond,
            at: Hex::new(1, 0),
            dir: 1,
            energy: ActivationEnergy::default(),
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
            energy: ActivationEnergy::default(),
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
    fn the_board_at_t0_is_one_source_one_bonder_one_first_tier_output_and_nothing_else() {
        let sim = start();
        let kinds: Vec<GlyphKind> = sim.glyphs.iter().flatten().map(|g| g.kind).collect();
        assert_eq!(
            kinds,
            [
                GlyphKind::Source,
                GlyphKind::Bonder,
                GlyphKind::Output(Tier::One)
            ]
        );
        assert!(sim.arms.is_empty());
        assert!(sim.atoms.is_empty());
        assert!(sim.bonds.is_empty());
        assert_eq!(sim.inventory, Inventory::EMPTY);
    }

    #[test]
    fn preloaded_world_crafts_an_arm_every_period_until_the_cap_holds() {
        let mut sim = preloaded();
        let period = 26;
        assert_eq!(sim.arms[0].tape.len(), period);
        for _ in 0..period * 3 {
            sim.step();
        }
        assert_eq!(count(&sim, Machine::Arm), 3 * (PLACEMENTS.len() as u32 - 1));
        let stalled: Vec<usize> = (0..sim.arms.len())
            .filter(|i| sim.arms[*i].stall.is_some())
            .collect();
        assert_eq!(stalled, vec![sim.arms.len() - 2, sim.arms.len() - 1]);
        for _ in 0..period {
            sim.step();
        }
        assert_eq!(count(&sim, Machine::Arm), DEFAULT_CAP);
        assert!(sim.atoms.len() < 60);
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
        let mut sim = bench(Vec::new(), vec![source(Hex::new(-1, 0))]);
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

        let mut sim = bench(vec![Instr::Move(2)], vec![source(Hex::new(1, -1))]);
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

    #[test]
    fn every_fixture_reaches_its_outcome_on_its_last_tick_and_not_before_with_no_arm_stalled() {
        for machine in Machine::ALL {
            let f = fixture(machine);
            assert!(f.ticks > 0, "{machine:?}");
            let mut sim = f.sim.clone();
            for tick in 0..f.ticks {
                assert!(!(f.done)(&sim), "{machine:?} done at tick {tick}");
                sim.step();
                for (i, arm) in sim.arms.iter().enumerate() {
                    assert_eq!(arm.stall, None, "{machine:?} arm {i} at tick {}", sim.tick);
                }
            }
            assert!((f.done)(&sim), "{machine:?}");
            assert_eq!(sim.tick, f.ticks, "{machine:?}");
        }
    }

    #[test]
    fn tick_zero_emits_the_exact_source_and_stall_events_in_simulation_order() {
        let mut sim = Sim::empty();
        sim.glyphs
            .push(Some(Glyph::new(GlyphKind::Source, Hex::new(2, 0), 0)));
        sim.arms.push(Arm::new(ORIGIN, 0, vec![Instr::Grab]));
        let tick = sim.step();
        assert_eq!(
            tick,
            TickEvents {
                tick: 0,
                events: vec![
                    TickEvent::Fired {
                        glyph: 0,
                        machine: Machine::Glyph(GlyphKind::Source),
                        at: Hex::new(2, 0),
                    },
                    TickEvent::Spawned {
                        glyph: 0,
                        atom: 0,
                        kind: AtomKind::Base,
                        at: Hex::new(2, 0),
                    },
                    TickEvent::Stalled {
                        arm: 0,
                        instruction: Instr::Grab,
                        reason: Stall::Illegal,
                    },
                ],
            }
        );
    }

    #[test]
    fn replay_returns_the_same_tick_events_and_state_as_stepping() {
        let initial = fixture(Machine::Glyph(GlyphKind::Bonder)).sim;
        let (replayed, events) = initial.replayed(9);
        let mut stepped = initial;
        let expected: Vec<TickEvents> = (0..9).map(|_| stepped.step()).collect();
        assert_eq!(events, expected);
        assert_eq!(replayed, stepped);
        assert!(events.iter().any(|tick| tick.events.iter().any(|event| {
            matches!(
                event,
                TickEvent::BondWritten {
                    kind: BondKind::Single,
                    ..
                }
            )
        })));
    }

    #[test]
    fn a_fixture_holds_only_its_machine_a_source_an_arm_and_what_they_need() {
        for machine in Machine::ALL {
            let f = fixture(machine);
            let kinds: Vec<GlyphKind> = f.sim.glyphs.iter().flatten().map(|g| g.kind).collect();
            match machine {
                Machine::Arm => assert_eq!((kinds, f.sim.arms.len()), (vec![], 1)),
                Machine::Glyph(kind) => {
                    assert_eq!(kinds.last(), Some(&kind), "{machine:?}");
                    assert!(kinds.iter().all(|k| *k == kind || *k == GlyphKind::Source));
                }
            }
        }
    }
}
