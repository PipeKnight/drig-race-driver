//! Supplied deformation node clipping, ordered strain and edge-length refresh.
use crate::{Error, spawn::reference_seed};

#[derive(Clone, Copy)]
pub struct Node {
    pub position: [f32; 4],
    pub rest: [f32; 4],
    pub bounds: [f32; 4],
}

#[derive(Clone, Copy)]
pub struct Edge {
    pub endpoints: [usize; 2],
    pub length: f32,
}

pub struct Input<'a> {
    pub dimensions: [usize; 3],
    pub spacing: f32,
    pub mode: u32,
    pub threshold: f32,
    pub gain: f32,
    /// Fixed400-record pool, including records outside sampled dimensions.
    pub nodes: &'a [Node],
    pub edges: &'a [Edge],
}

pub struct Output {
    pub positions: Vec<[f32; 4]>,
    pub lengths: Vec<f32>,
}

fn squared(a: [f32; 4], b: [f32; 4]) -> f32 {
    let d: [f32; 3] = std::array::from_fn(|i| a[i] - b[i]);
    (d[0] * d[0] + d[1] * d[1]) + d[2] * d[2]
}

pub fn calculate(input: Input<'_>) -> Result<Output, Error> {
    if input.dimensions.iter().any(|d| !(2..=16).contains(d))
        || input.dimensions.iter().product::<usize>() > 400
        || input.nodes.len() != 400
        || input.edges.len() > 1200
        || input.mode > 2
        || !input.spacing.is_finite()
        || input.spacing <= 0.
        || !input.threshold.is_finite()
        || !input.gain.is_finite()
        || input.nodes.iter().any(|n| {
            n.position
                .into_iter()
                .chain(n.rest)
                .chain(n.bounds)
                .any(|v| !v.is_finite())
                || n.bounds.iter().any(|v| *v < 0.)
        })
        || input
            .edges
            .iter()
            .any(|e| e.endpoints.iter().any(|v| *v >= 400) || !e.length.is_finite())
    {
        return Err(Error("invalid deformation settling input"));
    }
    let count = input.dimensions.iter().product::<usize>();
    let mut positions: Vec<_> = input.nodes.iter().map(|n| n.position).collect();
    for (position, node) in positions.iter_mut().zip(input.nodes).take(count) {
        for (i, value) in position.iter_mut().enumerate() {
            let lower = node.rest[i] - node.bounds[i];
            // SSE MAXPS/MINPS select the second operand at equal values.
            let clipped = if lower > *value { lower } else { *value };
            let upper = node.bounds[i] + node.rest[i];
            *value = if upper < clipped { upper } else { clipped };
        }
    }
    let mut degrees = [0u8; 400];
    let mut strain = [0f32; 400];
    for edge in input.edges {
        let [a, b] = edge.endpoints;
        let distance = squared(positions[a], positions[b]).sqrt();
        let amount = ((distance - input.spacing).abs() - input.threshold) * input.gain;
        degrees[a] = degrees[a].wrapping_add(1);
        degrees[b] = degrees[b].wrapping_add(1);
        strain[a] += amount;
        strain[b] += amount;
    }
    for (i, position) in positions.iter_mut().enumerate().take(count) {
        let degree = degrees[i] as i8;
        let quality = if degree > 0 {
            let value = 1. - strain[i] / degree as f32;
            if value > 0. { value } else { 0. }
        } else {
            1.
        };
        if position[3] > quality {
            position[3] = quality;
        }
    }
    let lower = input.spacing
        * f32::from_bits(if input.mode == 1 {
            0x3dcccccd
        } else {
            0x3e4ccccd
        });
    let upper = input.spacing * 2.;
    let mut lengths = Vec::with_capacity(input.edges.len());
    for edge in input.edges {
        let [a, b] = edge.endpoints;
        let length_squared = squared(positions[b], positions[a]);
        let length = if length_squared == 0. {
            0.
        } else {
            let seed = reference_seed(length_squared, false)?;
            let root = seed * length_squared;
            (1. - root * seed) * (root * 0.5) + root
        };
        lengths.push(if lower > length {
            lower
        } else if length > upper {
            upper
        } else {
            length
        });
    }
    if positions
        .iter()
        .flatten()
        .chain(&lengths)
        .any(|v| !v.is_finite())
    {
        return Err(Error("nonfinite deformation settling output"));
    }
    Ok(Output { positions, lengths })
}
