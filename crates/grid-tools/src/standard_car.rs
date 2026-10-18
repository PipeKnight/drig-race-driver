//! Owned cold Standard constructor, compared against whole original A0C9F0.
//! Outer factory, physical owner, resource association and spawn are separate.
use crate::standard_car_fields::{DIFFERENTIAL, EARLY, LATE};
use grid_formats::{vehicle::WheelGeometry, vehicle_properties::Properties};
use grid_world::{Error, dynamics, steering_geometry, suspension, vehicle_force, vehicle_gearing};

pub struct Car {
    bytes: Vec<u8>,
    pub driven: Vec<usize>,
    pub random_state: u32,
}
impl Car {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Install the five NED regions using the already selected local car offset.
    /// Wheel regions, mutable constructor defaults and impact dispatch are separate.
    pub fn configure_damage_regions(
        &mut self,
        document: &grid_formats::binxml::Document<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let regions = grid_formats::damage_boxes::read(document)?;
        let offset = std::array::from_fn(|i| self.get(0x1630 + i * 4));
        let mut records = [[0u8; 48]; 5];
        for ((record, region), at) in records
            .iter_mut()
            .zip(regions)
            .zip(grid_formats::damage_boxes::OFFSETS)
        {
            record.copy_from_slice(&self.bytes[0xcdf0 + at..0xcdf0 + at + 48]);
            grid_formats::damage_boxes::apply(region, offset, record)?;
        }
        for (record, at) in records.into_iter().zip(grid_formats::damage_boxes::OFFSETS) {
            self.bytes[0xcdf0 + at..0xcdf0 + at + 48].copy_from_slice(&record);
        }
        Ok(())
    }
    /// Prepare source suspension axes from raw NED positions and copied caster/
    /// camber properties. Preserve the complete owner on failure.
    pub fn configure_suspension(&mut self, geometry: WheelGeometry) -> Result<(), Error> {
        let mut body = std::array::from_fn(|i| u(&self.bytes, i * 4));
        for (i, axle) in [geometry.rear, geometry.rear, geometry.front, geometry.front]
            .into_iter()
            .enumerate()
        {
            for (j, value) in [
                axle.position[0] * if i % 2 == 0 { 1. } else { -1. },
                axle.position[1],
                axle.position[2],
                1.,
            ]
            .into_iter()
            .enumerate()
            {
                body[(0x14e0 + i * 16 + j * 4) / 4] = value.to_bits();
            }
        }
        let body = grid_world::suspension_context::initialize(&body)?;
        for (i, value) in body.into_iter().enumerate() {
            self.word(i * 4, value);
        }
        Ok(())
    }
    /// Install the complete source HDM settings, then run the selected original
    /// reset order. Commit only after every parameter and effect is valid.
    pub fn configure_handling(
        &mut self,
        settings: &grid_formats::handling_damage::Configuration,
        wheel_views_active: bool,
    ) -> Result<(), Error> {
        let mut owner = std::array::from_fn(|i| u(&self.bytes, 0xcdf0 + i * 4));
        owner[0x218 / 4..0x380 / 4].copy_from_slice(settings.words());
        let result =
            grid_world::vehicle_handling::reset(&owner, self.random_state, wheel_views_active)?;
        for (i, word) in result.owner.into_iter().enumerate() {
            self.word(0xcdf0 + i * 4, word);
        }
        self.random_state = result.random_state;
        Ok(())
    }
    /// The engine reset `996C70`, the first step of the original's car reset
    /// `A09CE0` (BRAKING_REFERENCE.md, F417). Engine +E0/+E4 (car +0x1880,
    /// +0x1884) scale the closed-throttle engine friction; the constructor
    /// leaves them zero, so without this reset the engine never brakes.
    pub fn reset_engine(&mut self) {
        for o in [0x1880, 0x1884, 0x1888, 0x1864] {
            self.set(o, 1.);
        }
        for o in [
            0x18c4, 0x189c, 0x18a0, 0x18a4, 0x1f60, 0x1894, 0x1890, 0x185c, 0x1878, 0x1898,
            0x1868,
        ] {
            self.set(o, 0.);
        }
        self.set(0x187c, 9999.);
        self.bytes[0x1860] = 0;
        self.bytes[0x1861] = 0;
        self.word(0x18c0, 0);
        self.word(0x1858, 0);
    }
    fn word(&mut self, offset: usize, value: u32) {
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn set(&mut self, offset: usize, value: f32) {
        self.word(offset, value.to_bits());
    }
    fn get(&self, offset: usize) -> f32 {
        f(&self.bytes, offset)
    }
    fn copy(&mut self, properties: &[u8], fields: &[(usize, usize, usize)]) {
        for &(size, dest, source) in fields {
            self.bytes[dest..dest + size].copy_from_slice(&properties[source..source + size]);
        }
    }
    fn shift(&mut self) -> Result<(), Error> {
        let mut frame = vehicle_force::Frame {
            body: std::array::from_fn(|i| u(&self.bytes, i * 4)),
            view: [0; vehicle_force::VIEW_WORDS],
            solver: [0; 884],
            driven: self.driven.clone(),
            material_multipliers: [None; 4],
            diagnostics: [0.; 8],
        };
        vehicle_gearing::initialize(&mut frame)?;
        for (i, value) in frame.body.into_iter().enumerate() {
            self.word(i * 4, value);
        }
        Ok(())
    }
}
fn u(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn f(b: &[u8], o: usize) -> f32 {
    f32::from_bits(u(b, o))
}

/// Whole selected cold Standard initialization. Source properties and geometry
/// must be independently selected. Owned links become body-relative tokens;
/// callback C784 is an identity token, never executable. Settings padding is zero.
/// The car setup's downforce slot (F414): the original applies the car's
/// default setup after construction (0xA0F260 → 0x9C3C30 default sliders →
/// 0x9E5680 → 0x9C32F0). For setup value 27 (downforce) the default slider is
/// `(clamp(x, B, A) − B) / (A − B)` with A, B the setup range (body +0xF24 and
/// +0xF9C, from CSV columns 245 and 244 in the explicit layout: 1.5 and 0.001)
/// and x the CSV downforce (+0xFD0), clamped to [−1, 1]; the applied value is
/// `(A − B)·s + B` (0 for a negative slider) and replaces +0x1F04; the drag
/// +0xE10 becomes `((downforce / +0xFD0)·0.2 + 0.8)·+0xFC4` when +0xFD0 ≠ 0.
/// A car whose CSV downforce lies inside the range keeps it (Lacetti 0.5,
/// e10 0x3F4CCCCD live); one above it is limited (Courage LC70 9 → 1.5,
/// e10 0x3EEAAAAC live) [V: both live captures]. The other 29 setup values
/// are not applied here (open).
pub fn apply_default_downforce_setup(owner: &mut [u32]) {
    let f = |o: usize| f32::from_bits(owner[o / 4]);
    let (a, b, x, drag_base) = (f(0xf24), f(0xf9c), f(0xfd0), f(0xfc4));
    // 0x9C3C30: NaN or below the range selects B, as the compares do.
    let slider = if b < a {
        let mut v = b;
        if b <= x {
            v = if a < x { a } else { x };
        }
        (v - b) / (a - b)
    } else {
        0.
    };
    // 0x9E5680 clamps every slider to [-1, 1].
    let slider = if -1. <= slider {
        if 1. < slider { 1. } else { slider }
    } else {
        -1.
    };
    let value = if 0. <= slider { (a - b) * slider + b } else { 0. };
    owner[0x1f04 / 4] = value.to_bits();
    if x != 0. {
        // 0xCB870C = 0.2, 0xCB0F24 = 0.8.
        let drag = ((value / x) * f32::from_bits(0x3e4c_cccd) + f32::from_bits(0x3f4c_cccd))
            * drag_base;
        owner[0xe10 / 4] = drag.to_bits();
    }
}

pub fn initialize(
    properties: &Properties,
    geometry: WheelGeometry,
    widths: [f32; 4],
    gravity: f32,
    random_state: u32,
) -> Result<Car, Error> {
    initialize_owned(properties, geometry, widths, gravity, random_state, false)
}

/// Selected observed outer-factory stores before whole Standard construction.
/// This is the properties/mass/identity-pose/activation prelude, not execution
/// of the original outer resource factory or player-preference application.
pub fn initialize_with_source_prelude(
    properties: &Properties,
    geometry: WheelGeometry,
    widths: [f32; 4],
    gravity: f32,
    random_state: u32,
) -> Result<Car, Error> {
    initialize_owned(properties, geometry, widths, gravity, random_state, true)
}

/// Selected outer mutable constructor followed by complete Standard creation.
/// The first reset consumes six global draws; Standard consumes twelve more.
/// Resource association and the later four wheel-region stores are separate.
pub fn initialize_with_mutable_prelude(
    properties: &Properties,
    geometry: WheelGeometry,
    widths: [f32; 4],
    gravity: f32,
    mut random_state: u32,
) -> Result<Car, Error> {
    for _ in 0..6 {
        random_state = random_state
            .wrapping_mul(0x41c64e6d)
            .wrapping_add(0x40584c0f);
    }
    let mut car =
        initialize_with_source_prelude(properties, geometry, widths, gravity, random_state)?;
    // Only these constructor defaults survive Standard's saved mutable block
    // and later reset. Whole original chain comparison verifies all car bytes.
    for at in [0x60, 0x90, 0xc0, 0xf0, 0x120, 0x150, 0x180, 0x1b0, 0x1e0] {
        car.set(0xcdf0 + at + 12, 1.);
    }
    for at in [0x430, 0x434, 0x438, 0x43c] {
        car.set(0xcdf0 + at, 1.);
    }
    car.bytes[0xcdf0 + 0x442] = 1;
    Ok(car)
}

/// Selected outer source wheel arrays, aliased Standard point storage and
/// original inline wheel damage-region stores. Full resource factory excluded.
pub fn initialize_with_outer_wheels(
    properties: &Properties,
    geometry: WheelGeometry,
    widths: [f32; 4],
    gravity: f32,
    random_state: u32,
) -> Result<Car, Error> {
    if widths
        .iter()
        .any(|v| !v.is_finite() || *v <= 0. || *v > 10.)
    {
        return Err(Error("invalid outer wheel widths"));
    }
    let radii = [
        geometry.rear.radius,
        geometry.rear.radius,
        geometry.front.radius,
        geometry.front.radius,
    ];
    let auxiliary = radii.map(|r| r * f32::from_bits(0x3f4c_cccd));
    let mut car =
        initialize_with_mutable_prelude(properties, geometry, auxiliary, gravity, random_state)?;
    for i in 0..4 {
        let wheel = 0x1990 + i * 0x150;
        let center: [f32; 4] = std::array::from_fn(|j| car.get(wheel + j * 4));
        for (j, value) in center.into_iter().enumerate() {
            car.set(0x14e0 + i * 16 + j * 4, value);
            car.set(0xcdf0 + 0x60 + i * 48 + j * 4, value);
        }
        for (at, value) in [
            (0x1520, radii[i]),
            (0x1530, auxiliary[i]),
            (0x1540, widths[i]),
        ] {
            car.set(at + i * 4, value);
        }
        for (j, value) in [widths[i], car.get(wheel + 0xe0), car.get(wheel + 0xe0), 0.]
            .into_iter()
            .enumerate()
        {
            car.set(0xcdf0 + 0x70 + i * 48 + j * 4, value);
        }
        car.bytes[0xcdf0 + 0x80 + i * 48] = 1;
    }
    Ok(car)
}

fn initialize_owned(
    properties: &Properties,
    geometry: WheelGeometry,
    widths: [f32; 4],
    gravity: f32,
    random_state: u32,
    source_prelude: bool,
) -> Result<Car, Error> {
    let p = properties.bytes();
    let n = |o| f(p, o);
    if u(p, 0x140) != 1
        || p[0x1d4] != 1
        || p[0x308] != 1
        || !(1..=3).contains(&u(p, 0x304))
        || !gravity.is_finite()
        || gravity < 0.
        || !n(0x130).is_finite()
        || n(0x130) <= 0.
        || n(0x130) > 1_000_000.
        || widths
            .iter()
            .any(|v| !v.is_finite() || *v <= 0. || *v > 10.)
        || [geometry.rear, geometry.front].iter().any(|a| {
            !a.radius.is_finite()
                || a.radius <= 0.
                || a.radius > 10.
                || a.position.iter().any(|v| !v.is_finite())
        })
    {
        return Err(Error("unsupported cold Standard constructor input"));
    }
    let mut car = Car {
        bytes: vec![0; 0x21000],
        driven: Vec::new(),
        random_state,
    };
    if source_prelude {
        car.bytes[0xe78..0x14d4].copy_from_slice(p);
        car.set(0x1560, 1.);
        car.word(0x1564, u(p, 0x3e0));
        for (o, value) in [(0xe48, 10), (0x9a8, 4), (0xba0, 2)] {
            car.word(o, value);
        }
        for o in [0xb13, 0xb14, 0x1ee0, 0x21c8] {
            car.bytes[o] = 1;
        }
        for base in [0x10, 0x15e0] {
            for i in 0..4 {
                car.set(base + i * 20, 1.);
            }
        }
        // Context9A4's owned self identity is zero at this pointer-free boundary.
    }
    car.copy(p, EARLY);
    car.set(0x15a4, 1. / n(0x130));
    for (o, value) in [
        (0x1f7c, 2.),
        (0x1f5c, 1.),
        (0x1f94, 1.),
        (0x1ef4, 1.),
        (0x1ef8, 1.),
        (0x1efc, 1.),
        (0x1f20, 1.),
        (0x1f14, 10.),
        (0x2214, 1.),
        (0x21c0, 1.),
    ] {
        car.set(o, value);
    }
    car.word(0x1598, 2);
    car.bytes[0xe6c] = 1;
    car.bytes[0xde4] = p[0x4d7];
    car.word(0xdf0, u32::MAX);
    if p[0x4d7] != 0 {
        car.word(0xdf8, 8);
        car.word(0xde8, 8);
        car.word(0xdec, 1);
    } else {
        car.word(0xdec, 3);
    }
    // Engine state; parsed-double defaults have already run in Properties.
    car.bytes[0x17ac..0x182c].copy_from_slice(&p[0x1f4..0x274]);
    for i in 0..7 {
        car.set(
            0x182c + i * 4,
            if n(0x274 + i * 4) < 1. {
                1.
            } else {
                n(0x274 + i * 4)
            },
        );
    }
    for (dest, source) in [
        (0x17a0, 0x1dc),
        (0x17a8, 0x1e0),
        (0x1854, 0x1f0),
        (0x17a4, 0x1e8),
        (0x1850, 0x2e4),
    ] {
        car.word(dest, u(p, source));
    }
    car.bytes[0x184c] = p[0x1d5];
    car.set(0x1874, 1.);
    car.set(0x1888, 1.);
    // Gear state before original setup reset and final slot-zero overwrite.
    car.bytes[0x18f4..0x191c].copy_from_slice(&p[0x318..0x340]);
    car.word(0x18f4, u(p, 0x31c));
    for (dest, source) in [(0x18cc, 0x310), (0x18d0, 0x30c), (0x18d4, 0x314)] {
        car.word(dest, u(p, source));
    }
    car.word(0x196c, 1);
    car.set(0x1974, 1.);
    car.bytes[0x198d] = 1;
    car.word(0x1620, 4);
    let mut points = [[0.; 4]; 4];
    let mut radii = [0.; 4];
    for (i, axle) in [geometry.rear, geometry.rear, geometry.front, geometry.front]
        .into_iter()
        .enumerate()
    {
        points[i] = [
            if i % 2 == 0 {
                axle.position[0]
            } else {
                -axle.position[0]
            },
            axle.position[1] - n(0x3e0),
            axle.position[2],
            1.,
        ];
        radii[i] = axle.radius;
        let w = 0x1990 + i * 0x150;
        car.word(w + 0x34, 0x15a0);
        car.word(w + 0xf0, u(p, 0x42c));
        let rear = i < 2;
        let fields = if rear {
            [
                (0x114, 0x43c),
                (0xe0, 0x464),
                (0x108, 0x3e8),
                (0x100, 0x3f0),
                (0x50, 0x3f8),
                (0x48, 0x400),
                (0x5c, 0x410),
                (0x60, 0x444),
                (0x70, 0x418),
                (0xe8, 0x428),
                (0x12c, 0x5b4),
                (0x58, 0x420),
                (0x68, 0x454),
                (0x64, 0x44c),
            ]
        } else {
            [
                (0x114, 0x438),
                (0xe0, 0x460),
                (0x108, 0x3e4),
                (0x100, 0x3ec),
                (0x50, 0x3f4),
                (0x48, 0x3fc),
                (0x5c, 0x40c),
                (0x60, 0x440),
                (0x70, 0x414),
                (0xe8, 0x424),
                (0x12c, 0x5b8),
                (0x58, 0x41c),
                (0x68, 0x450),
                (0x64, 0x448),
            ]
        };
        for (dest, source) in fields {
            car.word(w + dest, u(p, source));
        }
        car.set(
            w + 0x4c,
            if car.get(w + 0x50) <= 0.3 {
                0.3
            } else {
                car.get(w + 0x50)
            },
        );
        car.bytes[w + 0xfc] = u8::from(rear);
        car.bytes[w + 0x130] = p[if rear { 0x5bc } else { 0x5bd }];
        car.set(
            w + 0xe4,
            n(if rear { 0x408 } else { 0x404 }) * f32::from_bits(0x3f66_6666),
        );
        for offset in [0x20, 0x28, 0x74, 0xd4] {
            car.set(w + offset, 1.);
        }
        car.word(w + 0x13c, 0x4392_9333);
    }
    // The Standard constructor shifts the explicitly prepared source points.
    let span = ((points[0][0] - points[1][0]) + (points[2][0] - points[3][0])) * 0.5;
    let height = span * n(0x150) + n(0x3e0);
    let bottom = ((points[2][1] - radii[2] + (points[1][1] - radii[1]))
        + (points[3][1] - radii[3])
        + (points[0][1] - radii[0]))
        * 0.25;
    let rear_z = (points[0][2] + points[1][2]) * 0.5;
    let front_z = (points[2][2] + points[3][2]) * 0.5;
    let offset = [
        -0. - (((points[0][0] + points[1][0]) + points[2][0]) + points[3][0]) * 0.25,
        (-0. - height) - bottom,
        -0. - ((front_z - rear_z) * n(0x154) + rear_z),
        0.,
    ];
    for (i, point) in points.iter_mut().enumerate() {
        let w = 0x1990 + i * 0x150;
        for lane in 0..4 {
            *point.get_mut(lane).unwrap() += offset[lane];
            car.set(w + lane * 4, point[lane]);
        }
        car.set(w + 0xe0, radii[i]);
        car.set(w + 0xf4, radii[i]);
        car.set(w + 0xf8, widths[i]);
        car.set(w + 0x80, 1.);
    }
    for (lane, value) in offset.into_iter().enumerate() {
        car.set(0x1630 + lane * 4, value);
    }
    car.copy(p, DIFFERENTIAL);
    let drive = u(p, 0x304);
    if drive == 1 || drive == 2 {
        car.driven.extend([2, 3]);
    }
    if drive == 3 || drive == 2 {
        car.driven.extend([0, 1]);
    }
    for (slot, &index) in car.driven.clone().iter().enumerate() {
        car.word(0x18d8 + slot * 4, (0x1990 + index * 0x150) as u32);
    }
    car.word(0x18f0, car.driven.len() as u32);
    let weights = match drive {
        1 => [0., 0., 0.5, 0.5],
        3 => [0.5, 0.5, 0., 0.],
        _ => [
            n(0x518) * 0.5,
            n(0x518) * 0.5,
            (1. - n(0x518)) * 0.5,
            (1. - n(0x518)) * 0.5,
        ],
    };
    for (i, value) in weights.into_iter().enumerate() {
        car.set(0x1990 + i * 0x150 + 0x10c, value);
    }
    let preloads = suspension::rest_preloads(
        n(0x130),
        gravity,
        points.map(|p| p[2]),
        std::array::from_fn(|i| car.get(0x1990 + i * 0x150 + 0x5c)),
    )?;
    for (i, value) in preloads.into_iter().enumerate() {
        car.set(0x1990 + i * 0x150 + 0x54, value);
    }
    let steering = steering_geometry::initialize(points, n(0x340))?;
    car.set(0xe54, steering.scale_e54);
    car.word(0x21cc, (0x1990 + steering.pair[0] * 0x150) as u32);
    car.word(0x21d0, (0x1990 + steering.pair[1] * 0x150) as u32);
    car.set(0x21d4, steering.span_21d4);
    car.set(0x21d8, steering.span_21d8);
    car.shift()?;
    // Cold zero auxiliary settings: reset the separate owner and copy its saved
    // zero settings back. The temporary reset consumes six random samples even
    // though the temporary values are overwritten; retain that global sequence.
    car.bytes[0x22d0..0xd240].fill(0);
    for (o, value) in [
        (0xc720, 1.),
        (0xc788, 0.75),
        (0xc78c, 0.75),
        (0xc79c, 1.),
        (0xc7a8, 1.),
    ] {
        car.set(o, value);
    }
    car.set(0xc780, -1.);
    car.word(0xc7a0, u32::MAX);
    car.word(0xc784, 0x9a0c90);
    car.word(0x22ec, 4);
    car.word(0xc7f8, 0xd240);
    car.bytes[4] = 1;
    let mut sample = || {
        car.random_state = car
            .random_state
            .wrapping_mul(0x41c64e6d)
            .wrapping_add(0x40584c0f);
        let quotient = (car.random_state as i32) / 65535;
        (car.random_state.wrapping_add(quotient as u32) & 255) as f32
    };
    for _ in 0..6 {
        sample();
    }
    let rotations: [f32; 4] =
        std::array::from_fn(|_| (sample() * f32::from_bits(0x3c00_8081) - 1.) * 0.);
    let caps: [f32; 2] = std::array::from_fn(|_| -(sample() * f32::from_bits(0x3b80_8081)) + 1.);
    for (i, value) in rotations.into_iter().enumerate() {
        car.set(0xce40 + i * 4, value);
    }
    for (i, value) in caps.into_iter().enumerate() {
        car.set(0xce38 + i * 4, value);
    }
    car.word(0xcde0, u32::MAX);
    car.word(0xd218, u32::MAX);
    car.word(0xd234, 7);
    // Whole9C5B90, absent component, cold zero parameters. Its zero-width ramp
    // executes 0/0 in SSE and publishes this intermediate negative quiet NaN.
    // The later source factory must initialize the controls before driving.
    car.word(0xd19c, 0xffc0_0000);
    for o in [
        0xd194, 0xd198, 0xd1a0, 0xd1a4, 0xd1ac, 0xd1b8, 0xd1c8, 0xd1d0, 0xd1e0, 0xd1e8, 0xd1f8,
        0xd200, 0xd210,
    ] {
        car.set(o, 1.);
    }
    for (i, &radius) in radii.iter().enumerate() {
        let w = 0x1990 + i * 0x150;
        car.word(w + 0xe0, u(&car.bytes, w + 0xf4));
        for j in 0..5 {
            car.set(0x30c + i * 40 + j * 8, 0.);
            car.set(0x310 + i * 40 + j * 8, radius);
        }
    }
    // Whole default setup record; the two uninitialized native padding bytes
    // at settings5E/5F have explicit zero storage in this owned result.
    for i in 0..14 {
        car.set(0x1fd0 + i * 4, 0.5);
    }
    car.set(0x1fe8, n(0x3e4) / (n(0x3e8) + n(0x3e4)));
    car.word(0x2008, u(p, 0x424));
    car.word(0x200c, u(p, 0x428));
    car.set(0x2010, (n(0x534) + n(0x530)) * 0.5);
    car.set(0x2014, (n(0x52c) + n(0x528)) * 0.5);
    for i in 0..22 {
        car.word(0x2030 + i * 4, u32::MAX);
    }
    car.bytes[0x20e0..0x2108].copy_from_slice(&p[0x318..0x340]);
    car.set(0x2108, n(0x31c) * f32::from_bits(0x3f73_3333));
    let maximum = (0..10)
        .map(|i| n(0x318 + i * 4))
        .fold(0_f32, |a, b| if b > a { b } else { a });
    car.set(0x210c, maximum * f32::from_bits(0x3f93_3333));
    car.copy(p, LATE);
    for o in [0x21c4, 0x1f24] {
        car.set(o, 1.);
    }
    if car.get(0x1f64) == 0. {
        car.set(0x1f64, 0.5);
    }
    // Save original ratios, selected Y and denominator before setup reset.
    let saved = car.bytes[0x18f4..0x191c].to_vec();
    car.bytes[0x221c..0x2244].copy_from_slice(&saved);
    for i in 0..4 {
        let w = 0x1990 + i * 0x150;
        car.word(0x2244 + i * 4, u(&car.bytes, w + 4));
        car.word(0x2254 + i * 4, u(&car.bytes, w + 0x4c));
    }
    let impulse = dynamics::ImpulseBody {
        mass: car.get(0x15a0),
        inertia: std::array::from_fn(|i| car.get(0x15a8 + i * 4)),
        basis: std::array::from_fn(|r| std::array::from_fn(|c| f32::from(r == c))),
    };
    let force_point = [
        0.,
        0.,
        (1. - car.get(0x1f64)) * points[0][2] + points[2][2] * car.get(0x1f64),
        0.,
    ];
    let (linear, angular) =
        dynamics::apply_impulse(impulse, [0.; 4], [0.; 4], [0., -1., 0., 0.], force_point)?;
    for (dest, z) in [(0x1f8c, points[2][2]), (0x1f90, points[0][2])] {
        let v = dynamics::point_velocity(
            dynamics::BodyState {
                linear,
                angular,
                position: [0.; 4],
            },
            [0., 0., z, 0.],
        )?;
        car.set(dest, f32::from_bits(v[1].to_bits() ^ 0x8000_0000));
    }
    let mut peak = 0_f32;
    let mut power = 0_f32;
    for i in 1..32 {
        let value = car.get(0x17ac + i * 4);
        peak = peak.max(value);
        power = power
            .max(value / ((i as f32 * f32::from_bits(0x44d1_7084)) * f32::from_bits(0x3d04_2108)));
    }
    car.set(0x1870, peak);
    car.set(0x186c, power);
    car.word(0x1848, u(p, 0x538));
    car.bytes[0x21f2] = p[0x4d9];
    // Whole cold default setup restores original Y/denominators/ratios, then
    // writes settings gears (slot zero is the decoded zero, not the early alias).
    let final_gears = car.bytes[0x20e0..0x2108].to_vec();
    car.bytes[0x18f4..0x191c].copy_from_slice(&final_gears);
    car.set(0x1874, 1.);
    car.set(0x1f94, 1.);
    car.set(0x1ef4, 1.);
    car.set(0x1ef8, 1.);
    car.set(0x1efc, 1.);
    car.shift()?;
    Ok(car)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_last_region_preserves_car_rng_and_unrelated_fields() {
        use grid_formats::binxml::{Document, Element};
        let mut car = Car {
            bytes: vec![0; 0x21000],
            driven: vec![2, 3],
            random_state: 200679,
        };
        car.set(0x1634, -0.5);
        let attributes = vec![
            ("m_centre_x", "0"),
            ("m_centre_y", "1"),
            ("m_centre_z", "0"),
            ("m_size_x", "1"),
            ("m_size_y", "2"),
            ("m_size_z", "3"),
        ];
        let mut elements = vec![
            Element {
                name: "DAMAGE",
                value: None,
                attributes: vec![],
                children: 1..2,
                parent: None,
            },
            Element {
                name: "HANDLINGDAMAGE",
                value: None,
                attributes: vec![],
                children: 2..7,
                parent: Some(0),
            },
        ];
        for name in grid_formats::damage_boxes::NAMES {
            elements.push(Element {
                name,
                value: None,
                attributes: attributes.clone(),
                children: 7..7,
                parent: Some(1),
            });
        }
        elements[6].attributes.pop();
        let mut document = Document {
            strings: vec![],
            elements,
            string_padding: &[],
        };
        let before = car.bytes.clone();
        assert!(car.configure_damage_regions(&document).is_err());
        assert_eq!(car.bytes, before);
        assert_eq!(car.random_state, 200679);
        document.elements[6].attributes = attributes;
        car.configure_damage_regions(&document).unwrap();
        assert_eq!(car.random_state, 200679);
        let mut expected = before;
        for at in grid_formats::damage_boxes::OFFSETS {
            let start = 0xcdf0 + at;
            for (j, value) in [0_f32, 0.5, 0., 1., 1., 2., 3., 1.].into_iter().enumerate() {
                expected[start + j * 4..start + j * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
            expected[start + 32] = 1;
        }
        assert_eq!(car.bytes, expected);
    }
    #[test]
    fn failed_source_reset_preserves_car_and_rng() {
        let mut car = Car {
            bytes: vec![0; 0x21000],
            driven: vec![2, 3],
            random_state: 200679,
        };
        car.word(0xd19c, 0xffc0_0000);
        let before = car.bytes.clone();
        // Defaults have zero ramp start; explicitly make its end zero too.
        let bad = grid_formats::handling_damage::read_configuration(
            b"<VEHICLEHANDLINGDAMAGE><WHEEL damageToHaveFullEffect='0'/></VEHICLEHANDLINGDAMAGE>",
        )
        .unwrap();
        assert!(car.configure_handling(&bad, true).is_err());
        assert_eq!(car.bytes, before);
        assert_eq!(car.random_state, 200679);
        let good = grid_formats::handling_damage::read_configuration(b"<VEHICLEHANDLINGDAMAGE><BRAKE maxForceToCauseDamage='10016380'/><WHEEL damageToStartBaseEffect='.65'/></VEHICLEHANDLINGDAMAGE>").unwrap();
        car.configure_handling(&good, true).unwrap();
        assert!(car.get(0xd19c).is_finite());
        assert_eq!(car.get(0xcdf0 + 0x2f4), 10016380.);
        assert_ne!(car.random_state, 200679);
    }
}
