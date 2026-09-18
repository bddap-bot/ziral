use crate::form::{AtomRoute, CRAFT_RECIPE_COUNT, Form, atom_route, recipes};
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

    pub fn checked_add(self, o: Hex) -> Option<Hex> {
        Some(Hex::new(self.q.checked_add(o.q)?, self.r.checked_add(o.r)?))
    }

    pub fn sub(self, o: Hex) -> Hex {
        Hex::new(self.q - o.q, self.r - o.r)
    }

    pub fn scale(self, n: i32) -> Hex {
        Hex::new(self.q * n, self.r * n)
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
            Instr::Move(d) => (pivot.add(DIRS[(d + dir) % DIRS.len()]), dir),
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
    Copied {
        portal: usize,
        at: Hex,
    },
    Upgraded {
        glyph: usize,
        at: Hex,
    },
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
        length: ArmLength,
        atom: usize,
        at: Hex,
    },
    Dropped {
        arm: usize,
        length: ArmLength,
        atom: usize,
        at: Hex,
    },
    Rotated {
        arm: usize,
        length: ArmLength,
        spin: Spin,
        at: Hex,
    },
    Pivoted {
        arm: usize,
        length: ArmLength,
        spin: Spin,
        at: Hex,
    },
    Moved {
        arm: usize,
        length: ArmLength,
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
    Cobalt,
}

impl AtomKind {
    pub const ALL: [AtomKind; 4] = [
        AtomKind::Base,
        AtomKind::Amber,
        AtomKind::Plum,
        AtomKind::Cobalt,
    ];
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
    pub length: ArmLength,
    pub pivot: Hex,
    pub dir: usize,
    pub tape: Vec<Instr>,
    pub pc: usize,
    pub holding: bool,
    pub stall: Option<Stall>,
    pub energy: ActivationEnergy,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ArmLength {
    One,
    Two,
    Three,
}

impl ArmLength {
    pub const ALL: [ArmLength; 3] = [ArmLength::One, ArmLength::Two, ArmLength::Three];

    pub const fn cells(self) -> i32 {
        match self {
            ArmLength::One => 1,
            ArmLength::Two => 2,
            ArmLength::Three => 3,
        }
    }

    pub fn from_cells(cells: usize) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|length| length.cells() as usize == cells)
    }
}

impl Arm {
    pub fn new(length: ArmLength, pivot: Hex, dir: usize, tape: Vec<Instr>) -> Self {
        Arm {
            length,
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
        self.pivot.add(DIRS[self.dir].scale(self.length.cells()))
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

    pub fn cells(&self) -> Vec<Hex> {
        (0..=self.length.cells())
            .map(|n| self.pivot.add(DIRS[self.dir].scale(n)))
            .collect()
    }

    pub const fn machine(&self) -> Machine {
        Machine::Arm(self.length)
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
    SourceTwo,
    Bonder,
    SecondBond,
    Reification,
    Converter(AtomKind),
    Output(Tier),
}

impl GlyphKind {
    pub const fn is_source(self) -> bool {
        matches!(self, Self::Source | Self::SourceTwo)
    }

    pub const ALL: [GlyphKind; 11] = [
        GlyphKind::Source,
        GlyphKind::SourceTwo,
        GlyphKind::Bonder,
        GlyphKind::SecondBond,
        GlyphKind::Reification,
        GlyphKind::Converter(AtomKind::Amber),
        GlyphKind::Converter(AtomKind::Plum),
        GlyphKind::Converter(AtomKind::Cobalt),
        GlyphKind::Output(Tier::One),
        GlyphKind::Output(Tier::Two),
        GlyphKind::Output(Tier::Three),
    ];
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Machine {
    Portal,
    Arm(ArmLength),
    Glyph(GlyphKind),
}

impl Machine {
    pub const ALL: [Machine; GlyphKind::ALL.len() + ArmLength::ALL.len() + 1] = {
        let mut all =
            [Machine::Arm(ArmLength::One); GlyphKind::ALL.len() + ArmLength::ALL.len() + 1];
        let mut k = 0;
        while k < ArmLength::ALL.len() {
            all[k] = Machine::Arm(ArmLength::ALL[k]);
            k += 1;
        }
        k = 0;
        while k < GlyphKind::ALL.len() {
            all[k + ArmLength::ALL.len()] = Machine::Glyph(GlyphKind::ALL[k]);
            k += 1;
        }
        all[GlyphKind::ALL.len() + ArmLength::ALL.len()] = Machine::Portal;
        all
    };

    pub fn recipe(self) -> Option<&'static Form> {
        Item::Machine(self).recipe()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Item {
    Machine(Machine),
    Atom(AtomKind),
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
                    .map(|i| CRAFT_RECIPE_COUNT + i),
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
    pub kind: Option<AtomKind>,
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
        kind: Some(AtomKind::Base),
        consumed: false,
        lone: false,
    }
}

const fn any(at: Hex) -> Slot {
    Slot {
        kind: None,
        ..base(at)
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
    any(DIRS[0]),
    any(DIRS[1]),
];
const BONDER: [Slot; 2] = [any(ORIGIN), any(DIRS[0])];
const SOURCE: [Slot; 1] = [base(ORIGIN)];
const SOURCE_TWO: [Slot; 2] = [base(ORIGIN), base(DIRS[0])];
const SOURCE_TWO_BODY: [Hex; 4] = [
    Hex::new(1, -1),
    Hex::new(2, -1),
    Hex::new(0, 1),
    Hex::new(1, 1),
];
const SOURCE_BODY: [Hex; 5] = [DIRS[1], DIRS[2], DIRS[3], DIRS[4], DIRS[5]];
const AMBER_CONVERTER: [Slot; 3] = [consumed(ORIGIN), consumed(DIRS[0]), consumed(DIRS[1])];
const PLUM_CONVERTER: [Slot; 3] = [
    Slot {
        kind: Some(AtomKind::Amber),
        ..consumed(ORIGIN)
    },
    consumed(DIRS[0]),
    consumed(Hex::new(2, -1)),
];
const COBALT_CONVERTER: [Slot; 1] = [Slot {
    lone: true,
    ..consumed(ORIGIN)
}];
const COBALT_FOOTPRINT: [Hex; 4] = [ORIGIN, Hex::new(1, 0), Hex::new(0, 1), Hex::new(1, 1)];
const COBALT_OUTPUT: [Hex; 1] = [Hex::new(1, 1)];

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

const fn reification() -> [Slot; 19] {
    let mut slots = hexagon(Tier::Two.radius());
    let mut i = 0;
    while i < slots.len() {
        if slots[i].at.q == 0 && slots[i].at.r == 0 {
            slots[i].kind = None;
        }
        i += 1;
    }
    slots
}

const OUTPUT_1: [Slot; 7] = hexagon(Tier::One.radius());
const OUTPUT_2: [Slot; 19] = hexagon(Tier::Two.radius());
const OUTPUT_3: [Slot; 37] = hexagon(Tier::Three.radius());
const REIFICATION: [Slot; 19] = reification();

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
            GlyphKind::SourceTwo => plain(&SOURCE_TWO),
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
            GlyphKind::Converter(AtomKind::Cobalt) => plain(&COBALT_CONVERTER),
            GlyphKind::Converter(AtomKind::Base) => panic!("the base atom has a source"),
            GlyphKind::Output(Tier::One) => plain(&OUTPUT_1),
            GlyphKind::Output(Tier::Two) => plain(&OUTPUT_2),
            GlyphKind::Output(Tier::Three) => plain(&OUTPUT_3),
        }
    }

    pub fn cells(self) -> Vec<Hex> {
        match self {
            GlyphKind::Converter(AtomKind::Cobalt) => COBALT_FOOTPRINT.to_vec(),
            _ => self
                .rule()
                .slots
                .iter()
                .map(|slot| slot.at)
                .chain(self.body().iter().copied())
                .collect(),
        }
    }

    pub const fn body(self) -> &'static [Hex] {
        match self {
            GlyphKind::Source => &SOURCE_BODY,
            GlyphKind::SourceTwo => &SOURCE_TWO_BODY,
            _ => &[],
        }
    }

    const fn vacant(self) -> &'static [Hex] {
        match self {
            GlyphKind::Converter(AtomKind::Cobalt) => &COBALT_OUTPUT,
            _ => &[],
        }
    }

    pub const fn product(self) -> Option<Hex> {
        match self {
            GlyphKind::Converter(AtomKind::Amber | AtomKind::Plum) => Some(ORIGIN),
            GlyphKind::Converter(AtomKind::Cobalt) => Some(COBALT_OUTPUT[0]),
            _ => None,
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

    pub fn cells(&self) -> impl Iterator<Item = Hex> {
        let at = self.at;
        let dir = self.dir;
        self.kind
            .cells()
            .into_iter()
            .map(move |cell| at.add(cell.turned(dir)))
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
    count: [u32; CRAFT_RECIPE_COUNT + AtomKind::ALL.len()],
    cap: [u32; CRAFT_RECIPE_COUNT + AtomKind::ALL.len()],
}

impl Inventory {
    pub const EMPTY: Inventory = Inventory {
        count: [0; CRAFT_RECIPE_COUNT + AtomKind::ALL.len()],
        cap: [DEFAULT_CAP; CRAFT_RECIPE_COUNT + AtomKind::ALL.len()],
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
        self.cap
            .iter()
            .all(|cap| cap.is_power_of_two() && *cap <= MAX_CAP)
    }

    #[must_use]
    pub fn spend(&mut self, item: Item) -> bool {
        self.spend_all(&[item]).is_ok()
    }

    pub fn spend_all(&mut self, bill: &[Item]) -> Result<(), Vec<Short>> {
        let mut need = [0; CRAFT_RECIPE_COUNT + AtomKind::ALL.len()];
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
            let exponent = self.cap[i]
                .ilog2()
                .saturating_add_signed(notches)
                .min(MAX_CAP.ilog2());
            self.cap[i] = 1 << exponent;
        }
    }

    pub fn snap_caps(&mut self) -> bool {
        if self.cap.iter().any(|cap| *cap > MAX_CAP) {
            return false;
        }
        self.cap = self.cap.map(snap_cap);
        true
    }
}

fn snap_cap(cap: u32) -> u32 {
    let cap = cap.clamp(1, MAX_CAP);
    let lower = 1 << cap.ilog2();
    let upper = (lower << 1).min(MAX_CAP);
    if cap - lower < upper - cap {
        lower
    } else {
        upper
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Portal {
    pub at: Hex,
    pub sim: Sim,
}

impl Portal {
    pub fn new(at: Hex) -> Self {
        Self {
            at,
            sim: Sim::empty(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Sim {
    #[serde(default)]
    pub portals: Vec<Option<Portal>>,
    pub glyphs: Vec<Option<Glyph>>,
    pub arms: Vec<Arm>,
    pub atoms: Vec<Option<Atom>>,
    pub bonds: Vec<Bond>,
    pub tick: u64,
    pub inventory: Inventory,
    pub encountered: Vec<Item>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Id {
    Portal(usize),
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
            portals: Vec::new(),
            glyphs: Vec::new(),
            arms: Vec::new(),
            atoms: Vec::new(),
            bonds: Vec::new(),
            tick: 0,
            inventory: Inventory::EMPTY,
            encountered: Vec::new(),
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

    pub fn receive(&mut self, item: Item) {
        if let Some(i) = item.index() {
            self.inventory.count[i] = self.inventory.count[i].saturating_add(1);
            self.encounter(item);
        }
    }

    pub fn fill_inventory(&mut self) {
        self.inventory.count = self.inventory.cap;
        for item in recipes()
            .iter()
            .map(|(item, _)| *item)
            .chain(AtomKind::ALL.map(Item::Atom))
        {
            self.encounter(item);
        }
    }

    fn encounter(&mut self, item: Item) {
        if !self.encountered.contains(&item) {
            self.encountered.push(item);
        }
    }

    pub fn spawn(&mut self, atom: Atom) -> usize {
        self.encounter(Item::Atom(atom.kind));
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
        let arms = (0..self.arms.len()).map(Id::Arm).chain(
            self.portals
                .iter()
                .enumerate()
                .filter_map(|(i, p)| p.as_ref().map(|_| Id::Portal(i))),
        );
        let glyphs = self.glyphs.iter().enumerate();
        let atoms = self.atoms.iter().enumerate();
        arms.chain(glyphs.filter_map(|(i, g)| g.map(|_| Id::Glyph(i))))
            .chain(atoms.filter_map(|(i, a)| a.map(|_| Id::Atom(i))))
    }

    pub fn stands(&self, id: Id) -> impl Iterator<Item = Hex> + '_ {
        let (arm, glyph, atom) = match id {
            Id::Portal(i) => (Some(self.portals[i].as_ref().unwrap().at), None, None),
            Id::Arm(i) => (Some(self.arms[i].pivot), None, None),
            Id::Glyph(i) => (None, self.glyphs[i].as_ref(), None),
            Id::Atom(i) => (None, None, self.atoms[i].map(|a| a.pos)),
        };
        arm.into_iter()
            .chain(glyph.into_iter().flat_map(Glyph::cells))
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
                cell.checked_add(at)
                    .into_iter()
                    .flat_map(move |cell| self.on(cell).filter(move |other| !id.may_share(*other)))
            })
            .filter(move |other| !picked.contains(other))
    }

    pub fn fits(&self, set: &Sim, at: Hex, picked: &[Id]) -> bool {
        set.ids()
            .all(|id| set.stands(id).all(|cell| cell.checked_add(at).is_some()))
            && set.arms.iter().all(|arm| {
                arm.pivot
                    .checked_add(at)
                    .and_then(|pivot| pivot.checked_add(DIRS[arm.dir].scale(arm.length.cells())))
                    .is_some()
            })
            && self.blocked(set, at, picked).next().is_none()
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
            if g.kind.is_source() {
                let seats: Vec<_> = g.slots().filter(|at| self.atom_at(*at).is_none()).collect();
                if !seats.is_empty() {
                    events.push(TickEvent::Fired {
                        glyph: i,
                        machine: Machine::Glyph(g.kind),
                        at: g.at,
                    });
                }
                for at in seats {
                    let atom = self.spawn(Atom {
                        kind: AtomKind::Base,
                        pos: at,
                    });
                    events.push(TickEvent::Spawned {
                        glyph: i,
                        atom,
                        kind: AtomKind::Base,
                        at,
                    });
                }
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
                GlyphKind::Source | GlyphKind::SourceTwo => {}
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

    pub fn upgrade(&mut self, glyph: usize, compound: &Sim) -> Option<TickEvents> {
        let old = self.glyphs.get(glyph).copied().flatten()?;
        if old.kind != GlyphKind::Source
            || !compound.arms.is_empty()
            || compound.glyphs.iter().any(Option::is_some)
            || Form::of(compound) != *crate::form::source_upgrade()
        {
            return None;
        }
        let next = Glyph::new(GlyphKind::SourceTwo, old.at, old.dir);
        if next
            .kind
            .cells()
            .iter()
            .any(|cell| old.at.checked_add(cell.turned(old.dir)).is_none())
        {
            return None;
        }
        let mut grown = Sim::empty();
        grown.glyphs.push(Some(next));
        if !self.fits(&grown, ORIGIN, &[Id::Glyph(glyph)])
            || next
                .cells()
                .filter(|cell| !old.cells().any(|old| old == *cell))
                .any(|cell| self.atom_at(cell).is_some())
        {
            return None;
        }
        self.glyphs[glyph] = Some(Glyph {
            energy: ActivationEnergy::FULL,
            ..next
        });
        Some(TickEvents {
            tick: self.tick,
            events: vec![TickEvent::Upgraded { glyph, at: old.at }],
        })
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
            self.receive(item);
        }
    }

    fn matched(&self, g: Glyph) -> Option<Vec<usize>> {
        let rule = g.kind.rule();
        if g.kind
            .vacant()
            .iter()
            .map(|at| g.at.add(at.turned(g.dir)))
            .any(|at| self.atom_at(at).is_some())
        {
            return None;
        }
        let ids: Vec<usize> = g
            .slots()
            .zip(rule.slots)
            .map(|(at, slot)| {
                self.atom_at(at)
                    .filter(|id| {
                        slot.kind
                            .is_none_or(|kind| self.atoms[*id].unwrap().kind == kind)
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
            let AtomRoute::Converter(text) = atom_route(kind) else {
                panic!("a converter input")
            };
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
            self.receive(Item::Atom(kind));
        }
        if let GlyphKind::Converter(kind) = g.kind {
            let offset = g.kind.product().expect("a converter has an output");
            let at = g.at.add(offset.turned(g.dir));
            let atom = self.spawn(Atom { kind, pos: at });
            events.push(TickEvent::Spawned {
                glyph,
                atom,
                kind,
                at,
            });
        }
    }

    fn act(&mut self, i: usize, instr: Instr, events: &mut Vec<TickEvent>) -> bool {
        let from = self.arms[i].pivot;
        let held = self.held(i);
        let stall = self.exec(i, instr).err();
        self.arms[i].stall = stall;
        let length = self.arms[i].length;
        match stall {
            Some(reason) => events.push(TickEvent::Stalled {
                arm: i,
                instruction: instr,
                reason,
            }),
            None => match instr {
                Instr::Grab => events.push(TickEvent::Grabbed {
                    arm: i,
                    length,
                    atom: self.held(i).expect("a successful grab holds an atom"),
                    at: self.arms[i].pivot,
                }),
                Instr::Drop => {
                    if let Some(atom) = held {
                        events.push(TickEvent::Dropped {
                            arm: i,
                            length,
                            atom,
                            at: self.arms[i].pivot,
                        });
                    }
                }
                Instr::Rot(spin) => events.push(TickEvent::Rotated {
                    arm: i,
                    length,
                    spin,
                    at: self.arms[i].pivot,
                }),
                Instr::Pivot(spin) => events.push(TickEvent::Pivoted {
                    arm: i,
                    length,
                    spin,
                    at: self.arms[i].pivot,
                }),
                Instr::Move(_) => events.push(TickEvent::Moved {
                    arm: i,
                    length,
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
        let step = pivot.sub(arm.pivot);
        match instr {
            Instr::Wait => {}
            Instr::Grab => {
                self.atom_at(hand).ok_or(Stall::Illegal)?;
                self.arms[i].holding = true;
            }
            Instr::Drop => self.arms[i].holding = false,
            Instr::Rot(spin) => self.carry(i, pivot, |p| p.rotate(pivot, spin))?,
            Instr::Pivot(spin) => self.carry(i, pivot, |p| p.rotate(hand, spin))?,
            Instr::Move(_) => self.carry(i, pivot, |p| p.add(step))?,
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

    pub fn items(&self) -> impl Iterator<Item = Item> {
        let glyphs = self
            .glyphs
            .iter()
            .flatten()
            .map(|g| Item::Machine(Machine::Glyph(g.kind)));
        let arms = self.arms.iter().flat_map(|a| {
            std::iter::once(Item::Machine(a.machine()))
                .chain(a.tape.iter().map(|instr| Item::Token(*instr)))
        });
        let atoms = self
            .atoms
            .iter()
            .flatten()
            .map(|atom| Item::Atom(atom.kind));
        glyphs
            .chain(arms)
            .chain(
                self.portals
                    .iter()
                    .flatten()
                    .map(|_| Item::Machine(Machine::Portal)),
            )
            .chain(atoms)
    }

    pub fn bill(&self) -> Vec<Item> {
        let double = self
            .bonds
            .iter()
            .filter(|bond| bond.kind == BondKind::Double)
            .map(|_| Item::Atom(AtomKind::Base));
        self.items().chain(double).collect()
    }

    pub fn place(&mut self, other: &Sim, at: Hex) -> Vec<Id> {
        let mut placed: Vec<_> = (self.arms.len()..self.arms.len() + other.arms.len())
            .map(Id::Arm)
            .collect();
        for portal in other.portals.iter().flatten() {
            let mut portal = portal.clone();
            portal.at = portal.at.add(at);
            placed.push(Id::Portal(seat(&mut self.portals, portal)));
        }
        let glyphs = other.glyphs.iter().flatten().map(|g| {
            let g = Glyph {
                at: g.at.add(at),
                ..*g
            };
            Id::Glyph(seat(&mut self.glyphs, g))
        });
        placed.extend(glyphs);
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
        placed
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
    let mut sim = Sim::empty();
    let output = Hex::new(-2, 3);
    sim.glyphs
        .push(Some(Glyph::new(GlyphKind::Output(Tier::One), output, 0)));
    sim.arms
        .push(Arm::new(ArmLength::One, ORIGIN, 0, vec![Instr::Wait]));
    let recipe = Item::Machine(Machine::Arm(ArmLength::One))
        .recipe()
        .unwrap();
    let centre = recipe.centre(Tier::One.radius()).unwrap();
    sim.place(&recipe.sim(), output.sub(centre));
    sim
}

pub fn start() -> Sim {
    let mut sim = Sim::empty();
    let mut portal = Portal::new(Hex::new(2, 1));
    portal.sim = fixture(Machine::Arm(ArmLength::One)).sim;
    sim.portals.push(Some(portal));
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
    world
}

pub struct Fixture {
    pub sim: Sim,
    pub ticks: u64,
    pub done: fn(&Sim) -> bool,
}

fn armed(length: ArmLength, tape: Vec<Instr>) -> Sim {
    let mut sim = Sim::empty();
    sim.arms.push(Arm::new(length, ORIGIN, 0, tape));
    sim
}

fn crafted(s: &Sim) -> bool {
    s.arms.iter().all(|a| !a.holding)
        && s.atoms.iter().all(Option::is_none)
        && s.inventory.count.iter().sum::<u32>() == 1
}

fn arm_fixture_done(s: &Sim) -> bool {
    let arm = &s.arms[0];
    let drop = DIRS[1].scale(arm.length.cells());
    s.atom_at(drop).is_some() && !arm.holding
}

pub fn fixture(machine: Machine) -> Fixture {
    use Instr::{Drop, Grab, Move, Rot};
    let (cw, ccw) = (Rot(Spin::Cw), Rot(Spin::Ccw));
    let base = |pos| Atom {
        kind: AtomKind::Base,
        pos,
    };
    match machine {
        Machine::Portal => {
            let mut sim = Sim::empty();
            let mut portal = Portal::new(ORIGIN);
            portal.sim = fixture(Machine::Arm(ArmLength::One)).sim;
            sim.portals.push(Some(portal));
            Fixture {
                sim,
                ticks: 3,
                done: |s| s.tick == 3,
            }
        }
        Machine::Arm(length) => {
            let reach = DIRS[0].scale(length.cells());
            let mut sim = armed(length, vec![Grab, cw, Drop]);
            sim.spawn(base(reach));
            Fixture {
                sim,
                ticks: 3,
                done: arm_fixture_done,
            }
        }
        Machine::Glyph(kind @ (GlyphKind::Source | GlyphKind::SourceTwo)) => {
            let mut sim = Sim::empty();
            sim.glyphs.push(Some(Glyph::new(kind, ORIGIN, 0)));
            Fixture {
                sim,
                ticks: 1,
                done: |s| {
                    s.glyphs[0]
                        .unwrap()
                        .slots()
                        .all(|at| s.atom_at(at).is_some())
                },
            }
        }
        Machine::Glyph(GlyphKind::Bonder) => {
            let mut sim = armed(
                ArmLength::One,
                vec![
                    Move(0),
                    Grab,
                    Move(3),
                    cw,
                    cw,
                    Drop,
                    ccw,
                    ccw,
                    Move(0),
                    Grab,
                    Move(3),
                    cw,
                    Drop,
                ],
            );
            sim.glyphs
                .push(Some(Glyph::new(GlyphKind::Source, Hex::new(2, 0), 3)));
            sim.glyphs
                .push(Some(Glyph::new(GlyphKind::Bonder, DIRS[2], 0)));
            Fixture {
                sim,
                ticks: 13,
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
            let mut sim = armed(ArmLength::One, vec![Move(0), Grab, Move(3), cw, Drop]);
            sim.glyphs
                .push(Some(Glyph::new(GlyphKind::Source, Hex::new(2, 0), 3)));
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
                ticks: 5,
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
        Machine::Glyph(GlyphKind::Converter(AtomKind::Cobalt)) => {
            let glyph = Glyph::new(GlyphKind::Converter(AtomKind::Cobalt), ORIGIN, 0);
            let mut sim = Sim::empty();
            sim.spawn(base(ORIGIN));
            sim.glyphs.push(Some(glyph));
            Fixture {
                sim,
                ticks: 1,
                done: |s| {
                    s.atoms.iter().flatten().eq([&Atom {
                        kind: AtomKind::Cobalt,
                        pos: Hex::new(1, 1),
                    }])
                },
            }
        }
        Machine::Glyph(GlyphKind::Converter(AtomKind::Base)) => {
            panic!("the base atom has a source")
        }
        Machine::Glyph(GlyphKind::Output(tier)) => {
            let product = match tier {
                Tier::One => GlyphKind::Bonder,
                Tier::Two => GlyphKind::SecondBond,
                Tier::Three => GlyphKind::Converter(AtomKind::Amber),
            };
            let form = Machine::Glyph(product).recipe().unwrap();
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
            let mut sim = armed(ArmLength::One, vec![Grab, cw, Drop]);
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
    use crate::form::ATOM_ROUTES;

    #[test]
    fn tier_two_has_six_cells_and_two_independent_outlets_at_every_turn() {
        for dir in 0..6 {
            let at = Hex::new(4, -3);
            let g = Glyph::new(GlyphKind::SourceTwo, at, dir);
            let expected = [
                ORIGIN,
                DIRS[0],
                Hex::new(1, -1),
                Hex::new(2, -1),
                Hex::new(0, 1),
                Hex::new(1, 1),
            ];
            assert_eq!(
                g.cells().collect::<Vec<_>>(),
                expected.map(|h| at.add(h.turned(dir)))
            );
            let mut sim = Sim::empty();
            sim.glyphs.push(Some(g));
            let seats: Vec<_> = g.slots().collect();
            let tick = sim.step();
            assert_eq!(
                tick.events
                    .iter()
                    .filter(|e| matches!(e, TickEvent::Spawned { .. }))
                    .count(),
                2
            );
            assert!(seats.iter().all(|at| sim.atom_at(*at).is_some()));
            assert!(sim.step().events.is_empty());
            let id = sim.atom_at(seats[1]).unwrap();
            sim.consume(&[id]);
            assert_eq!(
                sim.step()
                    .events
                    .iter()
                    .filter(|e| matches!(e, TickEvent::Spawned { .. }))
                    .count(),
                1
            );
            assert_eq!(sim.atoms.iter().flatten().count(), 2);
        }
    }

    #[test]
    fn tier_two_emits_once_per_free_seat_and_fires_once_for_the_machine() {
        for dir in 0..6 {
            for mask in 0..4 {
                let mut sim = Sim::empty();
                let at = Hex::new(4, -3);
                let g = Glyph::new(GlyphKind::SourceTwo, at, dir);
                let seats = [at, at.add(DIRS[0].turned(dir))];
                assert_eq!(g.slots().collect::<Vec<_>>(), seats);
                sim.glyphs.push(Some(g));
                for (index, pos) in seats.into_iter().enumerate() {
                    if mask & (1 << index) != 0 {
                        sim.spawn(Atom {
                            kind: AtomKind::Base,
                            pos,
                        });
                    }
                }
                let tick = sim.step();
                assert_eq!(
                    tick.events
                        .iter()
                        .filter(|e| matches!(e, TickEvent::Fired { .. }))
                        .count(),
                    usize::from(mask != 3)
                );
                let actual: Vec<_> = tick
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        TickEvent::Spawned { at, .. } => Some(*at),
                        _ => None,
                    })
                    .collect();
                let expected: Vec<_> = seats
                    .into_iter()
                    .enumerate()
                    .filter_map(|(i, at)| (mask & (1 << i) == 0).then_some(at))
                    .collect();
                assert_eq!(actual, expected);
            }
        }
    }

    #[test]
    fn upgrade_rows_do_not_consume_inventory_slots() {
        assert_eq!(CRAFT_RECIPE_COUNT, 26);
        assert_eq!(
            Inventory::EMPTY.count(Machine::Glyph(GlyphKind::SourceTwo).into()),
            None
        );
        let count: Vec<u32> = (0..30).collect();
        let cap = vec![32u32; 30];
        let saved = serde_json::json!({ "count": count, "cap": cap });
        let inventory: Inventory = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(serde_json::to_value(inventory).unwrap(), saved);
        for (index, (item, _)) in recipes().iter().enumerate() {
            assert_eq!(inventory.count(*item), Some(index as u32));
        }
        for (index, kind) in AtomKind::ALL.into_iter().enumerate() {
            assert_eq!(inventory.count(Item::Atom(kind)), Some(26 + index as u32));
        }
    }

    #[test]
    fn source_upgrade_is_exact_and_transactional_at_every_turn() {
        let recipe = crate::form::source_upgrade();
        assert_eq!(
            recipe.to_string(),
            "P0,1 P0,2 P1,0 P1,2 P2,0 0,1=0,2 0,1=1,0 0,2=1,2 1,0=2,0"
        );
        assert_eq!(recipe.crafts(), None);
        assert_eq!(recipe.sim().component(0).len(), 5);
        for dir in 0..6 {
            let old = Glyph::new(GlyphKind::Source, Hex::new(3, 2), dir);
            let mut sim = Sim::empty();
            sim.glyphs.push(Some(old));
            let mut wrong = recipe.sim();
            wrong.atoms[0].as_mut().unwrap().kind = AtomKind::Base;
            let before = serde_json::to_vec(&sim).unwrap();
            assert!(sim.upgrade(0, &wrong).is_none());
            assert_eq!(serde_json::to_vec(&sim).unwrap(), before);
            let next = Glyph::new(GlyphKind::SourceTwo, old.at, dir);
            for cell in next
                .cells()
                .filter(|cell| !old.cells().any(|at| at == *cell))
            {
                for atom in [true, false] {
                    let mut blocked = sim.clone();
                    if atom {
                        blocked.spawn(Atom {
                            kind: AtomKind::Base,
                            pos: cell,
                        });
                    } else {
                        blocked.arms.push(Arm::new(ArmLength::One, cell, 0, vec![]));
                    }
                    let before = serde_json::to_vec(&blocked).unwrap();
                    assert!(blocked.upgrade(0, &recipe.sim()).is_none());
                    assert_eq!(serde_json::to_vec(&blocked).unwrap(), before);
                }
            }
            let event = sim.upgrade(0, &recipe.sim()).unwrap();
            assert_eq!(
                event.events,
                vec![TickEvent::Upgraded {
                    glyph: 0,
                    at: old.at
                }]
            );
            assert_eq!(sim.glyphs[0].unwrap().kind, GlyphKind::SourceTwo);
            assert_eq!(sim.glyphs[0].unwrap().dir, dir);
            assert!(sim.upgrade(0, &recipe.sim()).is_none());
            assert!(
                sim.inventory
                    .count(Machine::Glyph(GlyphKind::SourceTwo).into())
                    .is_none()
            );
        }
    }

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

        sim.arms
            .push(Arm::new(ArmLength::One, Hex::new(0, 0), 0, tape));
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
    fn one_tape_grabs_rotates_and_drops_at_each_arm_length() {
        use Instr::{Drop, Grab, Rot};
        let tape = vec![Grab, Rot(Spin::Cw), Drop];
        let mut sim = Sim::empty();
        for (i, length) in ArmLength::ALL.into_iter().enumerate() {
            let pivot = Hex::new(i as i32 * 8, 0);
            let arm = Arm::new(length, pivot, 0, tape.clone());
            sim.spawn(Atom {
                kind: AtomKind::Base,
                pos: arm.hand(),
            });
            sim.arms.push(arm);
        }
        sim.step();
        assert!(sim.arms.iter().all(|arm| arm.holding));
        sim.step();
        for (i, arm) in sim.arms.iter().enumerate() {
            assert_eq!(
                sim.atoms[sim.held(i).unwrap()].unwrap().pos,
                arm.pivot.add(DIRS[1].scale(arm.length.cells()))
            );
        }
        sim.step();
        for arm in &sim.arms {
            assert!(!arm.holding);
            assert!(
                sim.atom_at(arm.pivot.add(DIRS[1].scale(arm.length.cells())))
                    .is_some()
            );
        }
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
            ArmLength::One,
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
        sim.arms.push(Arm::new(
            ArmLength::One,
            Hex::new(2, 0),
            3,
            vec![Grab, Wait, Drop, Wait],
        ));
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
        sim.arms.push(Arm::new(
            ArmLength::One,
            Hex::new(2, -2),
            3,
            vec![Grab, Rot(Spin::Cw)],
        ));
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
    fn amber_bonds_to_each_base_of_the_reported_chain() {
        const REPORT: &str =
            "bug found, this A does not bond to the B\nA0,0\nB0,0 B0,1 B0,2 0,0-0,1 0,1=0,2";
        let forms: Vec<Form> = REPORT
            .lines()
            .skip(1)
            .map(|text| text.parse().unwrap())
            .collect();
        assert_eq!(forms.len(), 2);
        let amber = forms[0].sim();
        let chain = forms[1].sim();
        let targets: Vec<Hex> = chain.atoms.iter().flatten().map(|atom| atom.pos).collect();

        for target in targets {
            for amber_first in [false, true] {
                let mut sim = chain.clone();
                let (dir, at) = DIRS
                    .iter()
                    .enumerate()
                    .map(|(dir, step)| (dir, target.add(*step)))
                    .find(|(_, at)| sim.atom_at(*at).is_none())
                    .unwrap();
                sim.place(&amber, at);
                let base = sim.atom_at(target).unwrap();
                let amber = sim.atom_at(at).unwrap();
                let (bonder_at, bonder_dir) = if amber_first {
                    (at, (dir + 3) % 6)
                } else {
                    (target, dir)
                };
                sim.glyphs
                    .push(Some(Glyph::new(GlyphKind::Bonder, bonder_at, bonder_dir)));

                sim.step();

                let written = sim.bond_between(base, amber).unwrap();
                assert_eq!(
                    sim.bonds[written].kind,
                    BondKind::Single,
                    "{target:?} {amber_first}"
                );
                assert_eq!(sim.component(base).len(), 4, "{target:?} {amber_first}");
            }
        }
    }

    #[test]
    fn a_bond_that_would_pass_the_compound_cap_is_refused_byte_for_byte_and_one_that_meets_it_fires()
     {
        let left = MAX_COMPOUND_ATOMS as i32 - 2;
        let mut sim = bonder_joining_chains(left, 3);
        let before = sim.clone();
        let glyph = sim.glyphs[0].unwrap();
        let mut events = Vec::new();
        sim.fire(0, glyph, &mut events);
        assert!(events.is_empty());
        assert_eq!(sim, before);

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
    fn second_bonds_keep_atom_kinds_and_require_a_lone_base_fuel() {
        for left in AtomKind::ALL {
            for right in AtomKind::ALL {
                for fuel in AtomKind::ALL {
                    let mut sim = Sim::empty();
                    sim.glyphs
                        .push(Some(Glyph::new(GlyphKind::SecondBond, ORIGIN, 0)));
                    let feed = sim.spawn(Atom {
                        kind: fuel,
                        pos: ORIGIN,
                    });
                    let a = sim.spawn(Atom {
                        kind: left,
                        pos: DIRS[0],
                    });
                    let b = sim.spawn(Atom {
                        kind: right,
                        pos: DIRS[1],
                    });
                    bond(&mut sim, a, b, BondKind::Single);
                    sim.step();
                    assert_eq!(sim.atoms[a].unwrap().kind, left);
                    assert_eq!(sim.atoms[b].unwrap().kind, right);
                    assert_eq!(sim.atoms[feed].is_none(), fuel == AtomKind::Base);
                    assert_eq!(
                        sim.bonds[0].kind,
                        if fuel == AtomKind::Base {
                            BondKind::Double
                        } else {
                            BondKind::Single
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn every_recipe_is_one_distinct_compound_bonded_across_adjacent_cells_that_fits_its_tier() {
        let forms: Vec<&Form> = recipes().iter().map(|(_, form)| form).collect();
        for (k, (item, recipe)) in recipes().iter().enumerate() {
            let sim = recipe.sim();
            let tier = if matches!(
                *item,
                Item::Machine(
                    Machine::Arm(ArmLength::Two | ArmLength::Three)
                        | Machine::Glyph(
                            GlyphKind::Reification
                                | GlyphKind::Converter(AtomKind::Plum | AtomKind::Cobalt),
                        ),
                )
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
                !crate::form::RECIPES[..k]
                    .iter()
                    .any(|(other, _)| other == item),
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
        forms.extend(ATOM_ROUTES.iter().filter_map(|(_, route)| match route {
            AtomRoute::Source => None,
            AtomRoute::Converter(text) => Some(text.parse().unwrap()),
        }));
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

    #[test]
    fn the_cobalt_converter_changes_one_lone_base_at_its_input_into_one_cobalt_at_its_output() {
        let machine = Machine::Glyph(GlyphKind::Converter(AtomKind::Cobalt));
        let fixture = fixture(machine);
        let before = fixture.sim.clone();
        let after = fixture.sim.replay(1);
        assert!((fixture.done)(&after));
        assert_eq!(after.atoms.iter().flatten().count(), 1);
        assert_eq!(after.bonds, before.bonds);
        assert_eq!(after.glyphs.len(), before.glyphs.len());
        assert_eq!(after.arms, before.arms);
        assert_eq!(after.inventory, before.inventory);
    }

    #[test]
    fn the_cobalt_converter_leaves_wrong_inputs_and_a_blocked_output_untouched() {
        let kind = GlyphKind::Converter(AtomKind::Cobalt);
        for wrong in [AtomKind::Amber, AtomKind::Plum] {
            let mut sim = fixture(Machine::Glyph(kind)).sim;
            sim.atoms[0].as_mut().unwrap().kind = wrong;
            let before = sim.clone();
            sim.step();
            assert_eq!(sim.atoms, before.atoms, "{wrong:?} input");
            assert_eq!(sim.bonds, before.bonds, "{wrong:?} input");
        }
        let mut blocked = fixture(Machine::Glyph(kind)).sim;
        blocked.spawn(Atom {
            kind: AtomKind::Plum,
            pos: Hex::new(1, 1),
        });
        let before = blocked.clone();
        blocked.step();
        assert_eq!(blocked.atoms, before.atoms);
        assert_eq!(blocked.bonds, before.bonds);
    }

    #[test]
    fn the_cobalt_converter_occupies_its_four_cell_rhombus_at_every_turn() {
        let glyph = Glyph::new(GlyphKind::Converter(AtomKind::Cobalt), ORIGIN, 0);
        let expected = [ORIGIN, Hex::new(1, 0), Hex::new(0, 1), Hex::new(1, 1)];
        for turn in 0..6 {
            let turned = Glyph::new(glyph.kind, ORIGIN, turn)
                .cells()
                .collect::<Vec<_>>();
            assert_eq!(
                turned,
                expected.map(|cell| cell.turned(turn)),
                "turn {turn}"
            );
        }
    }

    fn mirrored(form: &Form) -> Form {
        let mut sim = form.sim();
        for atom in sim.atoms.iter_mut().flatten() {
            atom.pos = Hex::new(atom.pos.r, atom.pos.q);
        }
        Form::of(&sim)
    }

    #[test]
    fn every_token_is_one_cobalt_plus_at_most_two_base_atoms_and_distinct_through_turns_and_mirrors()
     {
        let tokens: Vec<(Instr, &Form)> = recipes()
            .iter()
            .filter_map(|(item, form)| match item {
                Item::Token(instr) => Some((*instr, form)),
                Item::Machine(_) | Item::Atom(_) => None,
            })
            .collect();
        assert_eq!(tokens.len(), 13);
        for (k, (instr, form)) in tokens.iter().enumerate() {
            assert_eq!(
                form.atoms()
                    .iter()
                    .filter(|(_, kind)| *kind == AtomKind::Cobalt)
                    .count(),
                1,
                "{instr:?}"
            );
            assert!(
                form.atoms()
                    .iter()
                    .all(|(_, kind)| matches!(kind, AtomKind::Base | AtomKind::Cobalt)),
                "{instr:?}"
            );
            assert!(form.atoms().len() <= 3, "{instr:?}");
            let sim = form.sim();
            for bond in sim
                .bonds
                .iter()
                .filter(|bond| bond.kind == BondKind::Double)
            {
                assert_eq!(sim.atoms[bond.a].unwrap().kind, AtomKind::Base, "{instr:?}");
                assert_eq!(sim.atoms[bond.b].unwrap().kind, AtomKind::Base, "{instr:?}");
            }
            let reflected = mirrored(form);
            for (other, other_form) in &tokens[..k] {
                assert_ne!(&reflected, *other_form, "{instr:?} mirrors {other:?}");
            }
        }
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
            sim.receive(Item::Atom(AtomKind::Base));
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
    fn an_arm_compound_crafts_and_an_extra_atom_or_bond_leaves_other_compounds_untouched() {
        let mut sim = bench(
            vec![Instr::Grab, Instr::Wait],
            vec![output(Tier::One, Hex::new(2, 0))],
        );
        let arm = Item::Machine(Machine::Arm(ArmLength::One))
            .recipe()
            .unwrap();
        let centre = arm.centre(Tier::One.radius()).unwrap();
        lay(&mut sim, arm, 0, Hex::new(2, 0).sub(centre));
        sim.step();
        assert_eq!(count(&sim, Machine::Arm(ArmLength::One)), 1);
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
        assert_eq!(count(&sim, Machine::Arm(ArmLength::One)), 1);
    }

    #[test]
    fn the_cobalt_arm_shape_crafts_at_an_output_and_the_base_shape_does_not() {
        let cobalt = Item::Machine(Machine::Arm(ArmLength::One))
            .recipe()
            .unwrap();
        let mut base = cobalt.sim();
        for atom in base.atoms.iter_mut().flatten() {
            atom.kind = AtomKind::Base;
        }
        let base = Form::of(&base);
        for (recipe, expected) in [(cobalt, 1), (&base, 0)] {
            let mut sim = Sim::empty();
            sim.glyphs.push(Some(output(Tier::One, ORIGIN)));
            let centre = recipe.centre(Tier::One.radius()).unwrap();
            let ids = lay(&mut sim, recipe, 0, ORIGIN.sub(centre));
            sim.step();
            assert_eq!(count(&sim, Machine::Arm(ArmLength::One)), expected);
            assert_eq!(lying(&sim, &ids), expected == 0);
        }
    }

    #[test]
    fn the_cap_holds_a_compound_until_the_count_drops_and_a_return_passes_it() {
        let bonder = Machine::Glyph(GlyphKind::Bonder);
        let recipe = bonder.recipe().unwrap();
        let mut sim = Sim::empty();
        sim.glyphs.push(Some(output(Tier::One, ORIGIN)));
        for _ in 1..DEFAULT_CAP {
            sim.receive(bonder.into());
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
        sim.receive(bonder.into());
        sim.receive(bonder.into());
        let ids = lay(&mut sim, recipe, 2, ORIGIN);
        sim.step();
        assert!(lying(&sim, &ids));
        assert_eq!(count(&sim, bonder), DEFAULT_CAP + 2);
        sim.inventory.set_cap(bonder.into(), 1);
        sim.step();
        assert!(!lying(&sim, &ids));
        assert_eq!(count(&sim, bonder), DEFAULT_CAP + 3);
        assert!(!sim.inventory.full(bonder.into()));
    }

    #[test]
    fn two_compounds_on_one_glyph_both_craft_in_one_tick() {
        let mut sim = Sim::empty();
        sim.glyphs.push(Some(output(Tier::Two, ORIGIN)));
        let bonder = Machine::Glyph(GlyphKind::Bonder);
        let pair = lay(&mut sim, bonder.recipe().unwrap(), 0, Hex::new(-2, 0));
        let arm = lay(
            &mut sim,
            Item::Machine(Machine::Arm(ArmLength::One))
                .recipe()
                .unwrap(),
            0,
            Hex::new(-1, -1),
        );
        sim.step();
        assert!(!lying(&sim, &pair) && !lying(&sim, &arm));
        assert_eq!(
            (
                count(&sim, bonder),
                count(&sim, Machine::Arm(ArmLength::One))
            ),
            (1, 1)
        );
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
        let dropper = Arm::new(ArmLength::One, Hex::new(0, 0), 0, vec![Grab, Drop, Wait]);
        let grabber = Arm::new(ArmLength::One, Hex::new(2, 0), 3, vec![Wait, Grab, Wait]);
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
        sim.arms.push(Arm::new(
            ArmLength::One,
            Hex::new(3, -1),
            3,
            vec![Grab, Wait, Drop, Wait],
        ));
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
        sim.arms.push(Arm::new(
            ArmLength::One,
            Hex::new(2, -2),
            4,
            vec![Grab, Wait, Drop, Wait],
        ));
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
    fn preloaded_world_crafts_each_arm_recipe_on_its_first_tick() {
        let mut sim = preloaded();
        let cobalt = sim
            .atoms
            .iter()
            .flatten()
            .filter(|atom| atom.kind == AtomKind::Cobalt)
            .count();
        assert_eq!(cobalt, 5 * PLACEMENTS.len());
        sim.step();
        assert_eq!(
            count(&sim, Machine::Arm(ArmLength::One)),
            PLACEMENTS.len() as u32
        );
    }

    #[test]
    fn a_move_translates_the_pose_and_the_six_moves_compose_to_a_ring() {
        let start = (Hex::new(2, -1), 3);
        let (pivot, dir) = Instr::Move(0).posed(start.0, start.1);
        assert_eq!((pivot, dir), (Hex::new(1, -1), 3));
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
            ArmLength::One,
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
        sim.arms.push(Arm::new(
            ArmLength::One,
            Hex::new(2, 0),
            3,
            vec![Grab, Wait, Drop, Wait],
        ));
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
        sim.arms
            .push(Arm::new(ArmLength::One, Hex::new(-1, 0), 0, Vec::new()));
        assert_eq!(base_move_onto(&mut sim), Some(Stall::Illegal));
        sim.arms.swap(0, 1);
        sim.arms[1].tape = vec![Instr::Move(3)];
        sim.step();
        assert_eq!(sim.arms[1].stall, Some(Stall::Illegal));
        assert_eq!(sim.arms[1].pivot, ORIGIN);

        let mut sim = bench(Vec::new(), Vec::new());
        put(&mut sim, -1, 0);
        assert_eq!(base_move_onto(&mut sim), Some(Stall::Illegal));

        let mut sim = bench(vec![Instr::Move(2)], vec![source(Hex::new(3, -1))]);
        sim.arms
            .push(Arm::new(ArmLength::One, Hex::new(-1, -1), 0, Vec::new()));
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
        sim.arms
            .push(Arm::new(ArmLength::One, Hex::new(1, 1), 0, Vec::new()));
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
    fn the_source_occupies_its_centre_and_five_neighbours_through_all_six_turns() {
        let at = Hex::new(3, -2);
        for dir in 0..6 {
            let source = Glyph::new(GlyphKind::Source, at, dir);
            let mut cells: Vec<Hex> = source.cells().collect();
            let mut expected: Vec<Hex> = std::iter::once(at)
                .chain(
                    DIRS.iter()
                        .enumerate()
                        .filter(|(i, _)| *i != dir)
                        .map(|(_, d)| at.add(*d)),
                )
                .collect();
            let key = |cell: &Hex| (cell.q, cell.r);
            cells.sort_by_key(key);
            expected.sort_by_key(key);
            assert_eq!(cells, expected, "turn {dir}");
            assert_eq!(source.slots().collect::<Vec<_>>(), [at], "turn {dir}");
        }
    }

    #[test]
    fn source_placement_is_refused_at_each_occupied_cell_and_fits_at_the_open_neighbour() {
        let source = Glyph::new(GlyphKind::Source, ORIGIN, 0);
        let mut set = Sim::empty();
        set.glyphs.push(Some(source));
        for cell in source.cells() {
            let mut world = Sim::empty();
            world
                .arms
                .push(Arm::new(ArmLength::One, cell, 0, Vec::new()));
            assert!(!world.fits(&set, ORIGIN, &[]), "{cell:?}");
        }
        let mut world = Sim::empty();
        world
            .arms
            .push(Arm::new(ArmLength::One, DIRS[0], 0, Vec::new()));
        assert!(world.fits(&set, ORIGIN, &[]));
    }

    #[test]
    fn output_fixtures_receive_three_distinct_products_other_than_their_own_recipes() {
        let mut products = Vec::new();
        for tier in [Tier::One, Tier::Two, Tier::Three] {
            let machine = Machine::Glyph(GlyphKind::Output(tier));
            let form = Form::of(&fixture(machine).sim);
            assert_ne!(&form, machine.recipe().unwrap(), "{tier:?} receives itself");
            let product = form.crafts().expect("the input has a recipe");
            assert!(!products.contains(&product), "{tier:?} repeats a product");
            products.push(product);
        }
    }

    #[test]
    fn output_fixtures_count_the_received_product_in_inventory() {
        for tier in [Tier::One, Tier::Two, Tier::Three] {
            let f = fixture(Machine::Glyph(GlyphKind::Output(tier)));
            let product = Form::of(&f.sim).crafts().unwrap();
            let mut sim = f.sim.replay(f.ticks);
            let mut received = Sim::empty();
            received.receive(product);
            assert_eq!(sim.inventory, received.inventory, "{tier:?} receipt");
            assert!((f.done)(&sim), "{tier:?} completes");
            sim.inventory = Inventory::EMPTY;
            assert!(!(f.done)(&sim), "{tier:?} requires receipt");
        }
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
        sim.arms
            .push(Arm::new(ArmLength::One, ORIGIN, 0, vec![Instr::Grab]));
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
        let fixture = fixture(Machine::Glyph(GlyphKind::Bonder));
        let initial = fixture.sim;
        let (replayed, events) = initial.replayed(fixture.ticks);
        let mut stepped = initial;
        let expected: Vec<TickEvents> = (0..fixture.ticks).map(|_| stepped.step()).collect();
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
                Machine::Portal => assert_eq!(f.sim.portals.iter().flatten().count(), 1),
                Machine::Arm(length) => {
                    assert_eq!((kinds, f.sim.arms.len()), (vec![], 1));
                    assert_eq!(f.sim.arms[0].length, length);
                }
                Machine::Glyph(kind) => {
                    assert_eq!(kinds.last(), Some(&kind), "{machine:?}");
                    assert!(kinds.iter().all(|k| *k == kind || *k == GlyphKind::Source));
                }
            }
        }
    }
}
