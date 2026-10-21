//! Rebuild cold, all-active impact groups from a supplied collision pool.
//! Geometry detection, existing links, sleeping bodies and parallel assignment
//! remain caller responsibilities. All identities below are owned one-based tokens.
use crate::{Error, joint::Joint};
use std::collections::VecDeque;

pub type Body = [u32; 0x424 / 4];
pub type Contact = [u32; 0x80 / 4];

/// A contact-pool slot before any producer writes it. The original pool
/// allocator `B19E20` builds every 0x80-byte slot with constructor `B0ECE0`:
/// +1C/+20 = -1, byte +0F (impact-pool flag) and byte +24 = 1, and zero
/// owners, shape tokens, limits, point, direction and byte +70. The vtable
/// word stays zero here like every other owned pointer. Wheel overtravel
/// keeps +0F, +1C..+24 from the slot, so a never-used slot needs this.
pub const EMPTY_CONTACT: Contact = {
    let mut c = [0; 0x80 / 4];
    c[0xc / 4] = 0x0100_0000;
    c[0x1c / 4] = u32::MAX;
    c[0x20 / 4] = u32::MAX;
    c[0x24 / 4] = 1;
    c
};

/// Consumed collision-shape bytes 216, 217, 229 and 230. Identity is the
/// one-based position in the caller's owned shape table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    pub angular: bool,
    pub flag217: bool,
    pub flag229: bool,
    pub flag230: bool,
}

/// A world joint (B825E0 links it into its bodies' constraint lists, body
/// +0x320 for A and +0x324 for B; here the link is the body ids).
#[derive(Clone)]
pub struct WorldJoint {
    pub joint: Joint,
    /// One-based body ids of A and of a dynamic B (0: none).
    pub a: u32,
    pub b: u32,
    /// B's record when B is a fixed object (joint +0xC).
    pub fixed: Option<Box<Body>>,
}

#[derive(Clone)]
pub struct Input {
    pub bodies: Vec<Body>,
    pub contacts: Vec<Contact>,
    pub active_order: Vec<usize>,
    pub selector: u32,
    /// In creation order (each body's list holds its joints newest first).
    pub joints: Vec<WorldJoint>,
}

#[derive(Clone)]
pub struct Output {
    pub bodies: Vec<Body>,
    pub contacts: Vec<Contact>,
    /// Group head, manager head, largest group, visited bodies, dirty head,
    /// new selector, new destination, active heads0/1 and active tails0/1.
    pub world: [u32; 11],
    /// The joints with their group's solver indices (+0x1C/+0x20) and +0xE.
    pub joints: Vec<WorldJoint>,
    /// Per root body: its group's unbroken joints in the group list's order.
    pub joint_groups: Vec<Vec<usize>>,
}

/// Retained ownership from the preceding completed world step. Contact links
/// may refer to consumed slots, so capacity is distinct from the active count.
pub(crate) struct Carry {
    pub dirty_head: u32,
    pub contact_capacity: usize,
}

/// Body +0x400 of an asleep body: in no active list (B0F2F0(-1), B24D70).
pub const ASLEEP: u32 = u32::MAX;

fn byte(row: &[u32], offset: usize) -> u8 {
    (row[offset / 4] >> (8 * (offset % 4))) as u8
}
fn put_byte(row: &mut [u32], offset: usize, value: u8) {
    let shift = 8 * (offset % 4);
    row[offset / 4] = (row[offset / 4] & !(255 << shift)) | (u32::from(value) << shift);
}
fn field(row: &[u32], offset: usize) -> u32 {
    row[offset / 4]
}
fn put(row: &mut [u32], offset: usize, value: u32) {
    row[offset / 4] = value;
}

pub fn validate(input: &Input) -> Result<(), Error> {
    validate_with_shapes(input, &[])
}

pub fn validate_with_shapes(input: &Input, shapes: &[Shape]) -> Result<(), Error> {
    validate_scope(input, shapes, None)
}

pub(crate) fn validate_carried(
    input: &Input,
    shapes: &[Shape],
    carry: &Carry,
) -> Result<(), Error> {
    validate_scope(input, shapes, Some(carry))
}

fn validate_scope(input: &Input, shapes: &[Shape], carry: Option<&Carry>) -> Result<(), Error> {
    let n = input.bodies.len();
    if !(1..=crate::MAX_WORLD_BODIES).contains(&n)
        || input.contacts.len() > crate::MAX_WORLD_CONTACTS
        || input.selector > 1
        || input.active_order.len() > n
        || shapes.len() > crate::MAX_WORLD_SHAPES
    {
        return Err(Error("invalid contact-group counts or selector"));
    }
    let mut seen = vec![false; n];
    for &i in &input.active_order {
        if i >= n || seen[i] {
            return Err(Error("invalid contact-group active permutation"));
        }
        seen[i] = true;
    }
    for (i, b) in input.bodies.iter().enumerate() {
        // An asleep body (B24D70) is in no active list: +0x400 is -1.
        let state = if seen[i] { input.selector } else { ASLEEP };
        let cold_links = [
            0, 4, 8, 0xc, 0x10, 0x14, 0x18, 0x1c, 0x2f0, 0x2f4, 0x2fc, 0x300, 0x304, 0x308, 0x30c,
            0x318,
        ];
        if [0x320, 0x324, 0x328, 0x32c, 0x418, 0x41c, 0x420]
            .iter()
            .any(|&o| field(b, o) != 0)
            || (carry.is_none() && cold_links.iter().any(|&o| field(b, o) != 0))
            || field(b, 0x334) != 0x1c0
            || field(b, 0x400) != state
            || byte(b, 0x2c) > 1
            || byte(b, 0x2d) != 0
            || byte(b, 0x412) > u8::from(carry.is_some())
            // Parked (B24DF0, +0x413) only while asleep.
            || byte(b, 0x413) > u8::from(!seen[i])
            || b[0x1c0 / 4..0x1d0 / 4]
                .iter()
                .any(|&x| !f32::from_bits(x).is_finite())
        {
            return Err(Error("unsupported contact-group body ownership"));
        }
    }
    if let Some(carry) = carry {
        if carry.contact_capacity > crate::MAX_WORLD_CONTACTS
            || input.contacts.len() > carry.contact_capacity
            || carry.dirty_head as usize > n
        {
            return Err(Error("invalid carried contact capacity or dirty head"));
        }
        for (i, b) in input.bodies.iter().enumerate() {
            if !seen[i] {
                continue;
            }
            if [
                8, 0xc, 0x10, 0x14, 0x18, 0x1c, 0x2f0, 0x2f4, 0x2fc, 0x300, 0x304, 0x308, 0x30c,
                0x318,
            ]
            .iter()
            .any(|&o| field(b, o) as usize > n)
                || [0, 4]
                    .iter()
                    .any(|&o| field(b, o) as usize > carry.contact_capacity)
            {
                return Err(Error("foreign carried group link"));
            }
            let at = input.active_order.iter().position(|&id| id == i).unwrap();
            if field(b, 0x300)
                != input
                    .active_order
                    .get(at + 1)
                    .map_or(0, |&id| id as u32 + 1)
                || field(b, 0x304)
                    != at
                        .checked_sub(1)
                        .map_or(0, |j| input.active_order[j] as u32 + 1)
            {
                return Err(Error("inconsistent carried active links"));
            }
        }
        let mut seen = vec![false; n];
        let mut id = carry.dirty_head;
        let mut previous = 0;
        while id != 0 {
            let i = id as usize - 1;
            if seen[i]
                || byte(&input.bodies[i], 0x412) != 1
                || field(&input.bodies[i], 0x2f4) != previous
            {
                return Err(Error("invalid carried dirty chain"));
            }
            seen[i] = true;
            previous = id;
            id = field(&input.bodies[i], 0x2f0);
        }
        if input.bodies.iter().enumerate().any(|(i, b)| {
            seen[i] != (byte(b, 0x412) == 1)
                || (!seen[i] && (field(b, 0x2f0) != 0 || field(b, 0x2f4) != 0))
        }) {
            return Err(Error("inconsistent carried dirty membership"));
        }
    }
    for j in &input.joints {
        let fixed = j.joint.byte(0xc) != 0;
        if j.a == 0
            || j.a as usize > n
            || j.b as usize > n
            || j.b == j.a
            || (fixed && j.b != 0)
            || (!fixed && j.fixed.is_some())
        {
            return Err(Error("unsupported contact-group joint"));
        }
    }
    for c in &input.contacts {
        let a = field(c, 4);
        let b = field(c, 8);
        let single = byte(c, 0xc);
        if a == 0
            || a as usize > n
            || b as usize > n
            || single > 1
            || (single == 0 && b != 0 && a == b)
            || field(c, 0x10) as usize > carry.map_or(0, |c| c.contact_capacity)
            || byte(c, 0xf) != 1
            || byte(c, 0x73) > 1
            || field(c, 0x28) as usize > shapes.len()
            || field(c, 0x2c) as usize > shapes.len()
            || (byte(c, 0x73) != 0 && (field(c, 0x28) == 0 || field(c, 0x2c) == 0))
            // Woken before grouping (B184F0, AD6C40); none reaches it.
            || field(&input.bodies[a as usize - 1], 0x400) == ASLEEP
            || (single == 0 && b != 0 && field(&input.bodies[b as usize - 1], 0x400) == ASLEEP)
            || c[0x50 / 4..0x70 / 4]
                .iter()
                .any(|&x| !f32::from_bits(x).is_finite())
        {
            return Err(Error("unsupported contact-group impact record"));
        }
    }
    Ok(())
}

struct Build {
    bodies: Vec<Body>,
    contacts: Vec<Contact>,
    /// Each root's joints, in the order the original's group list holds them.
    joint_lists: Vec<VecDeque<usize>>,
    heads: [u32; 2],
    tails: [u32; 2],
    group_head: u32,
}
impl Build {
    fn b(&self, id: u32, o: usize) -> u32 {
        field(&self.bodies[id as usize - 1], o)
    }
    fn set(&mut self, id: u32, o: usize, v: u32) {
        put(&mut self.bodies[id as usize - 1], o, v);
    }
    fn flag(&mut self, id: u32, o: usize, v: u8) {
        put_byte(&mut self.bodies[id as usize - 1], o, v);
    }
    fn root(&self, id: u32) -> u32 {
        if id == 0 || byte(&self.bodies[id as usize - 1], 0x2c) == 0 {
            0
        } else {
            self.b(id, 0x18)
        }
    }
    /// B0F2F0: unlink from the current list (none when asleep) and push at
    /// the head of `destination`.
    fn move_active(&mut self, id: u32, destination: u32) {
        let source = self.b(id, 0x400);
        if source == destination {
            return;
        }
        if source != ASLEEP {
            let next = self.b(id, 0x300);
            let previous = self.b(id, 0x304);
            if previous == 0 {
                self.heads[source as usize] = next;
            } else {
                self.set(previous, 0x300, next);
            }
            if next == 0 {
                self.tails[source as usize] = previous;
            } else {
                self.set(next, 0x304, previous);
            }
        }
        let head = self.heads[destination as usize];
        self.set(id, 0x304, 0);
        self.set(id, 0x300, head);
        self.heads[destination as usize] = id;
        if head == 0 {
            self.tails[destination as usize] = id;
        } else {
            self.set(head, 0x304, id);
        }
        self.set(id, 0x400, destination);
    }
    fn create(&mut self, id: u32) {
        for o in [0, 4, 0x14, 0x24] {
            self.set(id, o, 0);
        }
        for o in [8, 0xc, 0x18] {
            self.set(id, o, id);
        }
        let head = self.group_head;
        if head != 0 {
            self.set(head, 0x14, id);
        }
        self.set(id, 0x10, head);
        self.group_head = id;
        self.set(id, 0x20, 1);
        self.flag(id, 0x2c, 1);
    }
    fn attach(&mut self, root: u32, id: u32) {
        for o in [0, 4, 8, 0xc, 0x10, 0x14, 0x20, 0x24] {
            self.set(id, o, 0);
        }
        self.set(id, 0x18, root);
        self.flag(id, 0x2c, 1);
        self.set(self.b(root, 0xc), 0x2fc, id);
        self.set(root, 0xc, id);
        self.set(root, 0x20, self.b(root, 0x20) + 1);
    }
    fn merge(&mut self, a: u32, b: u32) {
        let previous = self.b(b, 0x14);
        let next = self.b(b, 0x10);
        if previous == 0 {
            self.group_head = next;
        } else {
            self.set(previous, 0x10, next);
        }
        if next != 0 {
            self.set(next, 0x14, previous);
        }
        self.set(self.b(a, 0xc), 0x2fc, self.b(b, 8));
        self.set(a, 0xc, self.b(b, 0xc));
        let last = self.b(a, 4);
        let head = self.b(b, 0);
        if last != 0 {
            put(&mut self.contacts[last as usize - 1], 0x10, head);
        }
        if self.b(a, 0) == 0 {
            self.set(a, 0, self.b(b, 0));
        }
        if self.b(b, 4) != 0 {
            self.set(a, 4, self.b(b, 4));
        }
        self.set(a, 0x20, self.b(a, 0x20) + self.b(b, 0x20));
        self.set(a, 0x24, self.b(a, 0x24) + self.b(b, 0x24));
        let moved = std::mem::take(&mut self.joint_lists[b as usize - 1]);
        self.joint_lists[a as usize - 1].extend(moved);
        let mut id = self.b(b, 8);
        while id != 0 {
            self.set(id, 0x18, a);
            id = self.b(id, 0x2fc);
        }
        for o in [0, 4, 8, 0xc, 0x10, 0x14, 0x20, 0x24] {
            self.set(b, o, 0);
        }
        self.set(b, 0x18, a);
    }
    fn contact(&mut self, index: usize, destination: u32) {
        let a = field(&self.contacts[index], 4);
        let b = if byte(&self.contacts[index], 0xc) == 0 {
            field(&self.contacts[index], 8)
        } else {
            0
        };
        self.join(a, b, destination);
        let root = self.root(a);
        let old = self.b(root, 0);
        put(&mut self.contacts[index], 0x10, old);
        self.set(root, 0, index as u32 + 1);
        self.set(root, 0x24, self.b(root, 0x24) + 1);
        if self.b(root, 4) == 0 {
            self.set(root, 4, index as u32 + 1);
        }
    }
    /// AD6250 for a joint: the same union, then the joint goes to the head of
    /// its group's list (kept apart from the contacts here: B7B850 sorts the
    /// list into separate contact and joint pools, keeping each one's order).
    fn joint(&mut self, index: usize, a: u32, b: u32, destination: u32) {
        self.join(a, b, destination);
        let root = self.root(a);
        self.joint_lists[root as usize - 1].push_front(index);
    }
    fn join(&mut self, a: u32, b: u32, destination: u32) {
        self.move_active(a, destination);
        if b != 0 {
            self.move_active(b, destination);
        }
        match (self.root(a), self.root(b)) {
            (0, 0) => {
                self.create(a);
                if b != 0 {
                    self.attach(a, b);
                }
            }
            (0, root) => self.attach(root, a),
            (root, 0) => {
                if b != 0 {
                    self.attach(root, b);
                }
            }
            (x, y) => {
                if x != y {
                    self.merge(x, y);
                }
            }
        }
    }
}

/// Whole selected serial group reconstruction; input state is immutable on error.
pub fn calculate(input: &Input) -> Result<Output, Error> {
    calculate_with_shapes(input, &[])
}

pub fn calculate_with_shapes(input: &Input, shapes: &[Shape]) -> Result<Output, Error> {
    calculate_with_geometry(input, shapes, input.contacts.len())
}

/// External pool records form the prefix and store points relative to body one.
/// Geometric pair records form the suffix and already store world-space points.
pub fn calculate_with_geometry(
    input: &Input,
    shapes: &[Shape],
    external_count: usize,
) -> Result<Output, Error> {
    validate_with_shapes(input, shapes)?;
    if external_count > input.contacts.len() {
        return Err(Error("invalid external contact prefix"));
    }
    build(input.clone(), shapes, 0, external_count, &mut 0)
}

/// B0F2F0 into an active list for a parked body (+0x413): it leaves the
/// parked list (head `parked`, links +0x308 next / +0x30C previous) and the
/// flag clears; its stale links stay, as in the original.
pub(crate) fn leave_parked(bodies: &mut [Body], parked: &mut u32, k: usize) {
    if byte(&bodies[k], 0x413) == 0 {
        return;
    }
    let (next, previous) = (field(&bodies[k], 0x308), field(&bodies[k], 0x30c));
    if previous == 0 {
        *parked = next;
    } else {
        put(&mut bodies[previous as usize - 1], 0x308, next);
    }
    if next != 0 {
        put(&mut bodies[next as usize - 1], 0x30c, previous);
    }
    put_byte(&mut bodies[k], 0x413, 0);
}

/// Carried-world grouping consuming its input: the bodies and contacts move
/// into the output instead of being copied (a world step's largest buffers).
pub(crate) fn calculate_carried_owned(
    input: Input,
    shapes: &[Shape],
    carry: &Carry,
    external_count: usize,
    parked: &mut u32,
) -> Result<Output, Error> {
    validate_carried(&input, shapes, carry)?;
    if external_count > input.contacts.len() {
        return Err(Error("invalid external contact prefix"));
    }
    build(input, shapes, carry.dirty_head, external_count, parked)
}

fn build(
    mut input: Input,
    shapes: &[Shape],
    mut dirty: u32,
    external_count: usize,
    parked: &mut u32,
) -> Result<Output, Error> {
    let n = input.bodies.len();
    // Read before the bodies move into the output (no step below changes
    // +0x400 before these are consulted).
    let asleep_flags: Vec<bool> =
        input.bodies.iter().map(|b| field(b, 0x400) == ASLEEP).collect();
    let mut out = Build {
        bodies: std::mem::take(&mut input.bodies),
        contacts: std::mem::take(&mut input.contacts),
        joint_lists: vec![VecDeque::new(); n],
        heads: [0; 2],
        tails: [0; 2],
        group_head: 0,
    };
    let source = input.selector as usize;
    let destination = 1 - input.selector;
    let ids: Vec<_> = input.active_order.iter().map(|&i| i as u32 + 1).collect();
    out.heads[source] = ids.first().copied().unwrap_or(0);
    out.tails[source] = ids.last().copied().unwrap_or(0);
    for (i, &id) in ids.iter().enumerate() {
        out.set(id, 0x300, ids.get(i + 1).copied().unwrap_or(0));
        out.set(id, 0x304, if i == 0 { 0 } else { ids[i - 1] });
        if byte(&out.bodies[id as usize - 1], 0x412) == 0 {
            out.set(id, 0x2f0, dirty);
            out.set(id, 0x2f4, 0);
            if dirty != 0 {
                out.set(dirty, 0x2f4, id);
            }
            dirty = id;
            out.flag(id, 0x412, 1);
        }
        out.flag(id, 0x2c, 0);
        out.set(id, 0x2fc, 0);
    }
    // AD6C40: each active body pushes its unbroken joints (its +0x320 list,
    // newest first) onto a stack; the stack is grouped before the contacts.
    // A joint reaches the stack through B's list (+0x324) only while A is
    // asleep; grouping it then moves A into the active list (AD6250: no
    // velocity or timer reset), so an awake body pulls its jointed partner.
    let asleep = |id: u32| asleep_flags[id as usize - 1];
    let mut stack = Vec::new();
    for &id in &ids {
        for (k, j) in input.joints.iter().enumerate().rev() {
            if j.a == id && j.joint.byte(0x48) == 0 {
                stack.push(k);
            }
        }
        for (k, j) in input.joints.iter().enumerate().rev() {
            if j.b == id && j.joint.byte(0xc) == 0 && j.joint.byte(0x48) == 0 && asleep(j.a) {
                stack.push(k);
            }
        }
    }
    // A pulled partner starts like the active bodies: island links cleared
    // and on the dirty chain.
    let mut pulled = vec![false; n];
    for &k in &stack {
        let id = input.joints[k].a;
        if asleep(id) && !std::mem::replace(&mut pulled[id as usize - 1], true) {
            // AD6250 moves it with B0F2F0, which also unparks it.
            leave_parked(&mut out.bodies, parked, id as usize - 1);
            if byte(&out.bodies[id as usize - 1], 0x412) == 0 {
                out.set(id, 0x2f0, dirty);
                out.set(id, 0x2f4, 0);
                if dirty != 0 {
                    out.set(dirty, 0x2f4, id);
                }
                dirty = id;
                out.flag(id, 0x412, 1);
            }
            out.flag(id, 0x2c, 0);
            out.set(id, 0x2fc, 0);
        }
    }
    for &k in stack.iter().rev() {
        let j = &input.joints[k];
        let b = if j.joint.byte(0xc) == 0 { j.b } else { 0 };
        out.joint(k, j.a, b, destination);
    }
    for i in 0..out.contacts.len() {
        if i < external_count {
            let id = field(&out.contacts[i], 4);
            for k in 0..4 {
                let value = f32::from_bits(out.b(id, 0x1c0 + k * 4))
                    + f32::from_bits(field(&out.contacts[i], 0x50 + k * 4));
                if !value.is_finite() {
                    return Err(Error("contact-group point overflow"));
                }
                put(&mut out.contacts[i], 0x50 + k * 4, value.to_bits());
            }
        }
        out.contact(i, destination);
    }
    let mut id = out.heads[source];
    while id != 0 {
        let next = out.b(id, 0x300);
        out.move_active(id, destination);
        out.create(id);
        id = next;
    }
    let mut joints = std::mem::take(&mut input.joints);
    let mut root = out.group_head;
    let mut manager = 0;
    let mut largest = 0;
    while root != 0 {
        largest = largest.max(out.b(root, 0x20));
        let mut id = out.b(root, 8);
        let mut index = 0;
        while id != 0 {
            out.set(id, 0x404, index);
            index += 1;
            id = out.b(id, 0x2fc);
        }
        let mut contact = out.b(root, 0);
        while contact != 0 {
            let i = contact as usize - 1;
            let c = &out.contacts[i];
            let a = out.b(field(c, 4), 0x404);
            let b = if byte(c, 0xc) == 0 && field(c, 8) != 0 {
                out.b(field(c, 8), 0x404)
            } else {
                u32::MAX
            };
            let disabled = if byte(c, 0x73) == 0 {
                true
            } else {
                let a = shapes[field(c, 0x28) as usize - 1];
                let b = shapes[field(c, 0x2c) as usize - 1];
                a.flag217 || b.flag217 || (a.flag229 && b.flag230) || (b.flag229 && a.flag230)
            };
            put(&mut out.contacts[i], 0x1c, a);
            put(&mut out.contacts[i], 0x20, b);
            put_byte(&mut out.contacts[i], 0xe, u8::from(disabled));
            contact = field(&out.contacts[i], 0x10);
        }
        out.set(root, 0x1c, manager);
        manager = root;
        // B82850 and AEEC80: the joint's solver indices, and it is enabled.
        for &k in &out.joint_lists[root as usize - 1] {
            let j = &mut joints[k];
            j.joint.words[0x1c / 4] = out.b(j.a, 0x404);
            j.joint.words[0x20 / 4] = if j.b != 0 && j.joint.byte(0xc) == 0 {
                out.b(j.b, 0x404)
            } else {
                u32::MAX
            };
            j.joint.set_byte(0xe, 0);
        }
        root = out.b(root, 0x10);
    }
    let world = [
        out.group_head,
        manager,
        largest,
        ids.len() as u32,
        dirty,
        destination,
        input.selector,
        out.heads[0],
        out.heads[1],
        out.tails[0],
        out.tails[1],
    ];
    Ok(Output {
        bodies: out.bodies,
        contacts: out.contacts,
        world,
        joints,
        joint_groups: out.joint_lists.into_iter().map(Vec::from).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometric_points_are_not_translated_as_external_pool_offsets() {
        let mut body = [0; 0x424 / 4];
        put(&mut body, 0x334, 0x1c0);
        for (i, v) in [10f32, 20., 30., 1.].into_iter().enumerate() {
            put(&mut body, 0x1c0 + i * 4, v.to_bits());
        }
        let mut contacts = vec![[0; 32]; 2];
        for (i, c) in contacts.iter_mut().enumerate() {
            put(c, 4, 1);
            put_byte(c, 0xc, 1);
            put_byte(c, 0xf, 1);
            let point = if i == 0 {
                [1f32, 2., 3., 0.]
            } else {
                [11., 22., 33., 1.]
            };
            for (j, v) in point.into_iter().enumerate() {
                put(c, 0x50 + j * 4, v.to_bits());
            }
        }
        let input = Input {
            bodies: vec![body],
            contacts,
            active_order: vec![0],
            selector: 0,
            joints: vec![],
        };
        let before = input.clone();
        let out = calculate_with_geometry(&input, &[], 1).unwrap();
        assert_eq!(out.contacts[0][20..24], out.contacts[1][20..24]);
        assert_eq!(out.contacts[1][20..24], before.contacts[1][20..24]);
        let relative = calculate(&input).unwrap();
        assert_ne!(relative.contacts[1][20..24], out.contacts[1][20..24]);
        assert!(calculate_with_geometry(&input, &[], 3).is_err());
        assert_eq!(input.bodies, before.bodies);
        assert_eq!(input.contacts, before.contacts);
    }

    #[test]
    fn late_point_overflow_keeps_complete_input_after_an_earlier_group() {
        let mut bodies = vec![[0; 0x424 / 4]; 2];
        for body in &mut bodies {
            put(body, 0x334, 0x1c0);
        }
        put(&mut bodies[1], 0x1c0, f32::MAX.to_bits());
        let mut contacts = vec![[0; 0x80 / 4]; 2];
        for (i, contact) in contacts.iter_mut().enumerate() {
            put(contact, 4, i as u32 + 1);
            put_byte(contact, 0xc, 1);
            put_byte(contact, 0xf, 1);
        }
        put(&mut contacts[1], 0x50, f32::MAX.to_bits());
        let input = Input {
            bodies,
            contacts,
            active_order: vec![1, 0],
            selector: 0,
            joints: vec![],
        };
        let before = input.clone();
        validate(&input).unwrap();
        assert_eq!(
            calculate(&input).err().unwrap().0,
            "contact-group point overflow"
        );
        assert_eq!(input.bodies, before.bodies);
        assert_eq!(input.contacts, before.contacts);
        assert_eq!(input.active_order, before.active_order);
        assert_eq!(input.selector, before.selector);
        let mut earlier = input.clone();
        earlier.contacts.truncate(1);
        let output = calculate(&earlier).unwrap();
        assert_eq!(output.bodies[0][0x24 / 4], 1);
        assert_ne!(output.contacts[0], before.contacts[0]);
    }
}
