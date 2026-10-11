//! Wheel overtravel contacts into a bounded, preallocated physical contact pool.
//! Selected car shape0 and static source shapes11 share material token0 and
//! recorded MAX friction limits. Allocation and later classification are external.
use crate::{Error, contact_groups::Contact};

#[derive(Clone, Debug, PartialEq)]
pub struct Pool {
    pub slots: Vec<Contact>,
    pub used: usize,
    /// Source static shapes; token1 is the car, source tokens start at2.
    pub shapes: usize,
}
/// Authored local point and world normal for the selected car/static producer.
/// Geometry queries and wheel-overtravel decisions remain the caller's work.
#[derive(Clone, Copy)]
pub struct Request {
    pub local: [f32; 4],
    pub direction: [f32; 4],
    pub penetration: f32,
    pub shape: u32,
}

pub(crate) fn validate_requests(requests: &[Request], shapes: usize) -> Result<(), Error> {
    if requests.len() > 4
        || !(1..=512).contains(&shapes)
        || requests.iter().any(|r| {
            !(2..=shapes as u32 + 1).contains(&r.shape)
                || r.local[3] != 1.
                || !r.penetration.is_finite()
                || r.local
                    .into_iter()
                    .chain(r.direction)
                    .any(|v| !v.is_finite())
        })
    {
        return Err(Error("invalid selected producer request"));
    }
    Ok(())
}

pub fn produce(
    physical: &[u32; 0x424 / 4],
    input: &Pool,
    requests: &[Request],
) -> Result<Pool, Error> {
    if input.slots.len() > 4
        || input.used > input.slots.len()
        || !(1..=512).contains(&input.shapes)
        || requests.len() > 4
    {
        return Err(Error("invalid selected producer pool"));
    }
    validate_requests(requests, input.shapes)?;
    let mut out = input.clone();
    for request in requests {
        let point = std::array::from_fn(|j| {
            let v = request.local;
            let world = (v[3] * f(physical, 0x200 + 4 * j) + v[1] * f(physical, 0x1e0 + 4 * j))
                + (v[0] * f(physical, 0x1d0 + 4 * j) + v[2] * f(physical, 0x1f0 + 4 * j));
            world - f(physical, 0x1c0 + 4 * j)
        });
        if point.iter().any(|v: &f32| !v.is_finite()) {
            return Err(Error("nonfinite selected producer point"));
        }
        if out.used == out.slots.len() {
            continue;
        }
        initialize(
            &mut out.slots[out.used],
            point,
            request.direction,
            request.penetration,
            request.shape,
        );
        out.used += 1;
    }
    Ok(out)
}

fn initialize(c: &mut Contact, point: [f32; 4], direction: [f32; 4], penetration: f32, token: u32) {
    for (o, v) in [
        (4, 1),
        (8, 0),
        (0x28, 1),
        (0x2c, token),
        (0x30, 0),
        (0x34, 0),
    ] {
        c[o / 4] = v;
    }
    for (o, v) in [(0xc, 0), (0x70, 0), (0x71, 0), (0x72, 1), (0x73, 1)] {
        set_byte(c, o, v);
    }
    c[0x38 / 4] = penetration.to_bits();
    c[0x3c / 4] = f32::MAX.to_bits();
    c[0x50 / 4..0x60 / 4].copy_from_slice(&point.map(f32::to_bits));
    c[0x60 / 4..0x70 / 4].copy_from_slice(&direction.map(f32::to_bits));
}

fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn byte(b: &[u32], o: usize) -> u8 {
    (b[o / 4] >> (o % 4 * 8)) as u8
}
fn set_byte(b: &mut [u32], o: usize, v: u8) {
    let shift = o % 4 * 8;
    b[o / 4] = (b[o / 4] & !(255 << shift)) | (u32::from(v) << shift);
}

/// Produce before the manager repairs the car basis. All pool slots, including
/// unused template bytes, are preserved on failure.
pub fn validate(car: &[u32], input: &Pool) -> Result<(), Error> {
    if car.len() != crate::vehicle_publication::VEHICLE_WORDS
        || car[0x1620 / 4] != 4
        || input.slots.len() > 4
        || input.used > input.slots.len()
        || !(1..=512).contains(&input.shapes)
        || car[0xdc8 / 4] > 1
        || car[0xdd8 / 4] != 0
        || byte(car, 0x1ee0) > 1
    {
        return Err(Error("invalid wheel contact ownership or pool"));
    }
    if (0..4).any(|i| {
        let t = car[(0x1a54 + i * 0x150) / 4];
        t != 0 && !(2..=input.shapes as u32 + 1).contains(&t)
    }) {
        return Err(Error("foreign wheel contact shape"));
    }
    Ok(())
}

pub fn calculate(car: &[u32], physical: &[u32; 0x424 / 4], input: &Pool) -> Result<Pool, Error> {
    validate(car, input)?;
    let mut out = input.clone();
    if byte(car, 0x1ee0) == 0 || car[0xdc8 / 4] == 0 {
        return Ok(out);
    }
    for i in 0..4 {
        let w = 0x1990 + i * 0x150;
        let distance = f(car, w + 0x40);
        let limit = f(car, w + 0x50);
        let token = car[(w + 0xc4) / 4];
        let crosses = distance > limit;
        if byte(car, 0x22e4 + i) != 0 || !crosses || token == 0 {
            continue;
        }
        let mut local: [f32; 4] = std::array::from_fn(|j| f(car, w + 4 * j));
        local[1] = (local[1] - f(car, w + 0xe0)) + (distance + limit) * 0.5;
        local[3] = 1.;
        let point: [f32; 4] = std::array::from_fn(|j| {
            let world = (local[3] * f(physical, 0x200 + j * 4)
                + local[1] * f(physical, 0x1e0 + j * 4))
                + (local[0] * f(physical, 0x1d0 + j * 4) + local[2] * f(physical, 0x1f0 + j * 4));
            world - f(physical, 0x1c0 + j * 4)
        });
        let direction: [f32; 4] = std::array::from_fn(|j| f(car, 0x15f0 + j * 4));
        let penetration = distance - limit;
        if local
            .into_iter()
            .chain(point)
            .chain(direction)
            .chain([penetration])
            .any(|x| !x.is_finite())
        {
            return Err(Error("nonfinite wheel contact"));
        }
        if out.used == out.slots.len() {
            continue;
        }
        initialize(
            &mut out.slots[out.used],
            point,
            direction,
            penetration,
            token,
        );
        out.used += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_point_capacity_and_fourth_wheel_failure_preserve_pool() {
        let mut car = vec![0; crate::vehicle_publication::VEHICLE_WORDS];
        let mut physical = [0; 0x424 / 4];
        car[0x1620 / 4] = 4;
        car[0xdc8 / 4] = 1;
        car[0x1ee0 / 4] = 1;
        car[0x15f4 / 4] = 1f32.to_bits();
        for i in 0..4 {
            physical[0x1d0 / 4 + i * 5] = 1f32.to_bits();
        }
        physical[0x1d0 / 4] = 2f32.to_bits();
        for (i, x) in [0.25f32, 0.5, 0.75, 1.].into_iter().enumerate() {
            physical[0x1c0 / 4 + i] = x.to_bits();
        }
        for i in 0..4 {
            let w = 0x1990 + i * 0x150;
            for (o, x) in [
                (0, 1f32),
                (4, 2.),
                (8, 3.),
                (0x40, 2.),
                (0x50, 1.),
                (0xe0, 0.5),
            ] {
                car[(w + o) / 4] = x.to_bits();
            }
            car[(w + 0xc4) / 4] = 2;
        }
        let pool = Pool {
            slots: vec![[0xdeadbeef; 32]; 4],
            used: 0,
            shapes: 1,
        };
        let good = calculate(&car, &physical, &pool).unwrap();
        assert_eq!(good.used, 4);
        assert_eq!(good.slots[0][0], 0xdeadbeef);
        assert_eq!(
            good.slots[0][0x50 / 4..0x60 / 4],
            [1.75f32, 2.5, 2.25, 0.].map(f32::to_bits)
        );
        let mut full = pool.clone();
        full.used = 4;
        assert_eq!(calculate(&car, &physical, &full).unwrap(), full);
        car[(0x1990 + 3 * 0x150) / 4] = f32::MAX.to_bits();
        let before = pool.clone();
        let mut requests = vec![
            Request {
                local: [1., 2., 3., 1.],
                direction: [0., 1., 0., 0.],
                penetration: 1.,
                shape: 2
            };
            4
        ];
        assert_eq!(produce(&physical, &pool, &requests).unwrap().used, 4);
        requests[3].local[0] = f32::MAX;
        assert_eq!(
            produce(&physical, &pool, &requests).unwrap_err().0,
            "nonfinite selected producer point"
        );
        assert_eq!(pool, before);
        assert_eq!(
            calculate(&car, &physical, &pool).unwrap_err().0,
            "nonfinite wheel contact"
        );
        assert_eq!(pool, before);
    }
}
