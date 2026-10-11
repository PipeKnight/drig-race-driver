//! Complete source car-to-body factory with one owned allocation and empty world.
//! Tokens describe ownership; resource shapes and runtime attachment are separate.
use grid_world::{Error, physical_constructor};
pub const CAR_BYTES: usize = 0x370a0;
pub const OUTPUT_BYTES: usize = CAR_BYTES + 0x430 + 16 + 0x100 + 0x1c + 16;

pub struct Output {
    pub car: Vec<u8>,
    pub physical: physical_constructor::Output,
    pub wrapper: [u32; 4],
    pub pool: [u32; 7],
    pub cell: [u32; 4],
}
fn word(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
/// Source 9B8D00 descriptor defaults and overrides, original pool allocation,
/// supplied-mass body construction and wrapper/backlink registration. The caller
/// supplies independently decoded source mass/inertia and a complete car owner.
pub fn initialize(
    car: &[u8],
    mass_inertia: [f32; 4],
    selector: u32,
    fill: u8,
) -> Result<Output, Error> {
    if car.len() != CAR_BYTES || word(car, 0xdb0) != 0 || word(car, 0xdd8) != 0 {
        return Err(Error("invalid source physical factory car owner"));
    }
    let mut settings = [0u32; 88];
    settings[0] = mass_inertia[0].to_bits();
    for (i, value) in mass_inertia[1..].iter().enumerate() {
        settings[(0x10 + 20 * i) / 4] = value.to_bits();
    }
    settings[0x4c / 4] = 1f32.to_bits();
    settings[0x60 / 4] = 1;
    for o in (0xa0..=0xb0).step_by(4).chain([0xec]) {
        settings[o / 4] = f32::MAX.to_bits();
    }
    for o in [0xe4, 0xe8, 0xf0, 0x104, 0x118, 0x12c] {
        settings[o / 4] = 1f32.to_bits();
    }
    settings[0x130 / 4] = 0x01010101;
    let mut physical = physical_constructor::initialize(&settings, selector, fill)?;
    physical.body[0x424 / 4] = 0x20000000;
    let mut car = car.to_vec();
    for o in (0xdb0..=0xdc8).step_by(4) {
        car[o..o + 4].fill(0);
    }
    car[0xdb0..0xdb4].copy_from_slice(&1u32.to_le_bytes());
    car[0x37090] = 0;
    Ok(Output {
        car,
        physical,
        wrapper: [0xd314e4, 0, 1, u32::from_le_bytes([fill; 4])],
        pool: [0, 0, 0, 1, 1, 16, 0],
        cell: [0x30000000, 0xcdcdcdcd, 0xcdcdcdcd, 0xcdcdcdcd],
    })
}
impl Output {
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = self.car.clone();
        b.extend(
            self.physical
                .body
                .iter()
                .chain(&self.wrapper)
                .chain(&self.physical.world)
                .chain(&self.pool)
                .chain(&self.cell)
                .flat_map(|v| v.to_le_bytes()),
        );
        b
    }
}
