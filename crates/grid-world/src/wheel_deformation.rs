//! Supplied-grid four-wheel visual geometry; collision/grid production is separate.
use crate::{Error, deformation_grid::Grid, pose, spawn::reference_seed};

#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub position: [f32; 4],
    pub current_radius: f32,
    pub normal_radius: f32,
    pub deflated_radius: f32,
    pub deflated: bool,
}

pub struct Input<'a> {
    pub grid: Grid<'a>,
    pub center: [f32; 4],
    pub scale: f32,
    pub angular_limit: f32,
    /// Selected by the sign of the wheel's original Z coordinate.
    pub radius_thresholds: [f32; 2],
    pub active: bool,
    pub wheels: [Wheel; 4],
    pub previous: Output,
}

#[derive(Clone, Copy, Debug)]
pub struct Output {
    pub displacement: [[f32; 4]; 4],
    pub angles: [f32; 4],
    /// Four matrices at body1690, not1680 (the fourth displacement).
    pub orientation: [[[f32; 4]; 4]; 4],
    pub radius_samples: [[[f32; 2]; 5]; 4],
    pub deflated: [bool; 4],
}

fn cross(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
        0.,
    ]
}

fn squared(v: [f32; 4]) -> f32 {
    (v[1] * v[1] + v[0] * v[0]) + (v[3] * v[3] + v[2] * v[2])
}

fn normalized(v: [f32; 4]) -> Result<[f32; 4], Error> {
    let length = squared(v);
    if !length.is_finite() || length < f32::MIN_POSITIVE {
        return Err(Error("degenerate deformation direction"));
    }
    let seed = reference_seed(length, false)?;
    let inverse = (1. - (seed * length) * seed) * (seed * 0.5) + seed;
    Ok(v.map(|v| inverse * v))
}

pub fn calculate(input: Input<'_>) -> Result<Output, Error> {
    if input.grid.dimensions.iter().any(|d| !(2..=16).contains(d))
        || input.grid.dimensions.iter().product::<usize>() > 400
        || input.grid.positions.len() != input.grid.dimensions.iter().product::<usize>()
        || !input.grid.spacing.is_finite()
        || input.grid.spacing <= 0.
        || input
            .grid
            .minimum
            .iter()
            .chain(input.grid.positions.iter().flatten())
            .any(|v| !v.is_finite())
    {
        return Err(Error("invalid supplied deformation descriptor"));
    }
    if input
        .center
        .into_iter()
        .chain([input.scale, input.angular_limit])
        .chain(input.radius_thresholds)
        .chain(input.wheels.iter().flat_map(|w| {
            w.position
                .into_iter()
                .chain([w.current_radius, w.normal_radius, w.deflated_radius])
        }))
        .chain(input.previous.displacement.into_iter().flatten())
        .chain(input.previous.angles)
        .chain(input.previous.orientation.into_iter().flatten().flatten())
        .chain(
            input
                .previous
                .radius_samples
                .into_iter()
                .flatten()
                .flatten(),
        )
        .any(|v| !v.is_finite())
        || input
            .wheels
            .iter()
            .any(|w| w.current_radius <= 0. || w.normal_radius <= 0. || w.deflated_radius <= 0.)
    {
        return Err(Error("invalid wheel deformation input"));
    }
    let mut result = input.previous;
    for (i, wheel) in input.wheels.into_iter().enumerate() {
        result.deflated[i] = wheel.deflated;
        result.displacement[i] = [0.; 4];
        if input.active {
            let center: [f32; 4] =
                std::array::from_fn(|lane| wheel.position[lane] - input.center[lane]);
            let sample = |axis, positive| {
                let mut query = center;
                for (lane, value) in query.iter_mut().enumerate() {
                    let offset = if lane == axis {
                        wheel.current_radius
                    } else {
                        0.
                    };
                    *value = if positive {
                        *value + offset
                    } else {
                        *value - offset
                    };
                }
                input.grid.sample(query)
            };
            let positive_z = sample(2, true)?;
            let negative_z = sample(2, false)?;
            let mut displacement = std::array::from_fn(|lane| {
                (0.5 * (positive_z[lane] + negative_z[lane]) - center[lane]) * input.scale
            });
            displacement[1] = 0.;
            displacement[3] = 0.;
            result.displacement[i] = displacement;
            let positive_y = sample(1, true)?;
            let negative_y = sample(1, false)?;
            let z_direction = std::array::from_fn(|lane| negative_z[lane] - positive_z[lane]);
            let y_direction = std::array::from_fn(|lane| positive_y[lane] - negative_y[lane]);
            let normal = normalized(cross(z_direction, y_direction))?;
            if normal[0] >= f32::from_bits(0x3f7ffcb9) {
                result.angles[i] = 0.;
                result.orientation[i] = [
                    [1., 0., 0., 0.],
                    [0., 1., 0., 0.],
                    [0., 0., 1., 0.],
                    [0., 0., 0., 1.],
                ];
            } else {
                let angle = (f64::from(normal[0]).acos() as f32) * input.scale;
                let axis = normalized(cross([1., 0., 0., 0.], normal))?;
                let orientation_angle = if input.angular_limit != 0.
                    && angle < input.angular_limit * f32::from_bits(0x3c8efa35)
                {
                    angle * 0.5
                } else {
                    angle
                };
                result.angles[i] = angle;
                result.orientation[i] = pose::rotation(axis, orientation_angle)?;
            }
            let length_squared = squared(displacement);
            let length = if length_squared == 0. {
                0.
            } else {
                let seed = reference_seed(length_squared, false)?;
                let root = seed * length_squared;
                (1. - root * seed) * (0.5 * root) + root
            };
            let threshold = input.radius_thresholds[usize::from(wheel.position[2] <= 0.)];
            if length > threshold {
                result.deflated[i] = true;
            }
        }
        let radius = if result.deflated[i] {
            wheel.deflated_radius
        } else {
            wheel.current_radius
        };
        result.radius_samples[i] = [[0., radius]; 5];
    }
    if result
        .displacement
        .iter()
        .flatten()
        .chain(&result.angles)
        .chain(result.orientation.iter().flatten().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(Error("nonfinite wheel deformation result"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inactive_preserves_angles_and_orientation_and_rebuilds_every_cache_pair() {
        let positions = [[0.; 4]; 8];
        let previous = Output {
            displacement: [[3.; 4]; 4],
            angles: [-0., 1., 2., 3.],
            orientation: [[[2.; 4]; 4]; 4],
            radius_samples: [[[5.; 2]; 5]; 4],
            deflated: [false; 4],
        };
        let wheels = std::array::from_fn(|i| Wheel {
            position: [0.; 4],
            current_radius: 0.3,
            normal_radius: 0.4,
            deflated_radius: 0.2,
            deflated: i % 2 == 0,
        });
        let out = calculate(Input {
            grid: Grid {
                dimensions: [2; 3],
                spacing: 1.,
                minimum: [0.; 4],
                positions: &positions,
            },
            center: [0.; 4],
            scale: 1.,
            angular_limit: 27.,
            radius_thresholds: [1.; 2],
            active: false,
            wheels,
            previous,
        })
        .unwrap();
        assert_eq!(out.displacement, [[0.; 4]; 4]);
        assert_eq!(
            out.angles.map(f32::to_bits),
            previous.angles.map(f32::to_bits)
        );
        assert_eq!(out.orientation, previous.orientation);
        for i in 0..4 {
            assert_eq!(
                out.radius_samples[i],
                [[0., if i % 2 == 0 { 0.2 } else { 0.3 }]; 5]
            );
        }
        assert_eq!(out.deflated, [true, false, true, false]);
    }
}
