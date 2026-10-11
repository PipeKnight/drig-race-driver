//! Rebuild cold, all-active impact groups from a supplied collision pool.
//! Geometry detection, existing links, sleeping bodies and parallel assignment
//! remain caller responsibilities. All identities below are owned one-based tokens.
use crate::Error;

pub type Body = [u32; 0x424 / 4];
pub type Contact = [u32; 0x80 / 4];

/// Consumed collision-shape bytes 216, 217, 229 and 230. Identity is the
/// one-based position in the caller's owned shape table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    pub angular: bool,
    pub flag217: bool,
    pub flag229: bool,
    pub flag230: bool,
}

#[derive(Clone)]
pub struct Input {
    pub bodies: Vec<Body>,
    pub contacts: Vec<Contact>,
    pub active_order: Vec<usize>,
    pub selector: u32,
}

pub struct Output {
    pub bodies: Vec<Body>,
    pub contacts: Vec<Contact>,
    /// Group head, manager head, largest group, visited bodies, dirty head,
    /// new selector, new destination, active heads0/1 and active tails0/1.
    pub world: [u32; 11],
}

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
    let n = input.bodies.len();
    if !(1..=16).contains(&n)
        || input.contacts.len() > 64
        || input.selector > 1
        || input.active_order.len() != n
        || shapes.len() > 513
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
    for b in &input.bodies {
        if [
            0, 4, 8, 0xc, 0x10, 0x14, 0x18, 0x1c, 0x2f0, 0x2f4, 0x2fc, 0x300, 0x304, 0x308, 0x30c,
            0x318, 0x320, 0x324, 0x328, 0x32c, 0x418, 0x41c, 0x420,
        ]
        .iter()
        .any(|&o| field(b, o) != 0)
            || field(b, 0x334) != 0x1c0
            || field(b, 0x400) != input.selector
            || byte(b, 0x2c) > 1
            || byte(b, 0x2d) != 0
            || byte(b, 0x412) != 0
            || byte(b, 0x413) != 0
            || b[0x1c0 / 4..0x1d0 / 4]
                .iter()
                .any(|&x| !f32::from_bits(x).is_finite())
        {
            return Err(Error("unsupported contact-group body ownership"));
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
            || field(c, 0x10) != 0
            || byte(c, 0xf) != 1
            || byte(c, 0x73) > 1
            || field(c, 0x28) as usize > shapes.len()
            || field(c, 0x2c) as usize > shapes.len()
            || (byte(c, 0x73) != 0 && (field(c, 0x28) == 0 || field(c, 0x2c) == 0))
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
    fn move_active(&mut self, id: u32, destination: u32) {
        let source = self.b(id, 0x400);
        if source == destination {
            return;
        }
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
        let root = self.root(a);
        let old = self.b(root, 0);
        put(&mut self.contacts[index], 0x10, old);
        self.set(root, 0, index as u32 + 1);
        self.set(root, 0x24, self.b(root, 0x24) + 1);
        if self.b(root, 4) == 0 {
            self.set(root, 4, index as u32 + 1);
        }
    }
}

/// Whole selected serial group reconstruction; input state is immutable on error.
pub fn calculate(input: &Input) -> Result<Output, Error> {
    calculate_with_shapes(input, &[])
}

pub fn calculate_with_shapes(input: &Input, shapes: &[Shape]) -> Result<Output, Error> {
    validate_with_shapes(input, shapes)?;
    let mut out = Build {
        bodies: input.bodies.clone(),
        contacts: input.contacts.clone(),
        heads: [0; 2],
        tails: [0; 2],
        group_head: 0,
    };
    let source = input.selector as usize;
    let destination = 1 - input.selector;
    let ids: Vec<_> = input.active_order.iter().map(|&i| i as u32 + 1).collect();
    out.heads[source] = ids[0];
    out.tails[source] = *ids.last().unwrap();
    let mut dirty = 0;
    for (i, &id) in ids.iter().enumerate() {
        out.set(id, 0x300, ids.get(i + 1).copied().unwrap_or(0));
        out.set(id, 0x304, if i == 0 { 0 } else { ids[i - 1] });
        out.set(id, 0x2f0, dirty);
        out.set(id, 0x2f4, 0);
        if dirty != 0 {
            out.set(dirty, 0x2f4, id);
        }
        dirty = id;
        out.flag(id, 0x412, 1);
        out.flag(id, 0x2c, 0);
        out.set(id, 0x2fc, 0);
    }
    for i in 0..out.contacts.len() {
        let id = field(&out.contacts[i], 4);
        for k in 0..4 {
            let value = f32::from_bits(out.b(id, 0x1c0 + k * 4))
                + f32::from_bits(field(&out.contacts[i], 0x50 + k * 4));
            if !value.is_finite() {
                return Err(Error("contact-group point overflow"));
            }
            put(&mut out.contacts[i], 0x50 + k * 4, value.to_bits());
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
