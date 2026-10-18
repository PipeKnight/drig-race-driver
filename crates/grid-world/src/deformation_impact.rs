//! Supplied impact records, ordered grid constraints and mean-motion removal.
//! Collision-to-record production and live update cadence remain separate.
use crate::{Error, deformation_settle, spawn::reference_seed};

#[derive(Clone, Copy)]
pub struct Node {
    pub position: [f32; 4],
    pub rest: [f32; 4],
    pub bounds: [f32; 4],
    pub motion: [f32; 4],
    pub marker: f32,
}

#[derive(Clone, Copy)]
pub struct Record {
    pub direction: [f32; 4],
    pub anchor: [f32; 4],
    pub node: usize,
    pub scalar: f32,
}

pub struct Input<'a> {
    pub dimensions: [usize; 3],
    pub spacing: f32,
    pub mode: u32,
    pub strain_threshold: f32,
    pub strain_gain: f32,
    pub displacement_threshold: f32,
    pub supplied_gain: f32,
    pub integration_step: f32,
    pub damping: f32,
    pub motion_scale: f32,
    pub iterations: u32,
    pub counter: u32,
    pub clip_each_iteration: bool,
    pub offset: [f32; 4],
    pub dirty: bool,
    pub nodes: &'a [Node],
    pub edges: &'a [deformation_settle::Edge],
    pub records: &'a [Record],
}

pub struct Output {
    pub nodes: Vec<Node>,
    pub lengths: Vec<f32>,
    pub offset: [f32; 4],
    pub counter: u32,
    pub dirty: bool,
}

fn mean(sum: [f32; 4], count: usize) -> Result<[f32; 4], Error> {
    let count = count as f32;
    let seed = reference_seed(count, true)?;
    Ok(sum.map(|v| {
        let product = seed * v;
        (1. - seed * count) * product + product
    }))
}

fn clip(node: &mut Node) {
    for i in 0..4 {
        let lower = node.rest[i] - node.bounds[i];
        let value = if lower > node.position[i] {
            lower
        } else {
            node.position[i]
        };
        let upper = node.bounds[i] + node.rest[i];
        node.position[i] = if upper < value { upper } else { value };
    }
}

#[allow(clippy::assign_op_pattern)] // Preserve observed factor/position-first SSE operand order.
pub fn calculate(input: Input<'_>) -> Result<Output, Error> {
    // Nodes the step reads and writes: the grid, padded to the four-node
    // unroll. The packed edge list pads its last group of four with edges
    // between the spare node 399 (F401: the Skyline grid, 6x5x13 nodes and 892
    // edges); those edges give that spare node a nonfinite motion, which no
    // grid node reads, so the motion checks cover only the grid nodes.
    let used = input.dimensions.iter().product::<usize>().div_ceil(4) * 4;
    if input.dimensions.iter().any(|d| !(2..=16).contains(d))
        || input.dimensions.iter().product::<usize>() > 400
        || input.nodes.len() != 400
        || input.edges.len() > 1200
        || input.records.len() > 512
        || input.mode > 2
        || !(1..=64).contains(&input.iterations)
        || input.counter > 64
        || input.spacing <= 0.
        || input.supplied_gain.abs() > 100.
        || [
            input.spacing,
            input.strain_threshold,
            input.strain_gain,
            input.displacement_threshold,
            input.supplied_gain,
            input.integration_step,
            input.damping,
            input.motion_scale,
        ]
        .into_iter()
        .chain(input.offset)
        .any(|v| !v.is_finite())
        || input.nodes.iter().enumerate().any(|(i, n)| {
            n.position
                .into_iter()
                .chain(n.rest)
                .chain(n.bounds)
                .chain([n.marker])
                .any(|v| !v.is_finite())
                || (i < used && n.motion.iter().any(|v| !v.is_finite()))
                || n.bounds.iter().any(|v| *v < 0.)
        })
        || input
            .edges
            .iter()
            .any(|e| e.endpoints.iter().any(|v| *v >= 400) || !e.length.is_finite())
        || input.records.iter().any(|r| {
            r.node >= 400
                || r.direction
                    .into_iter()
                    .chain(r.anchor)
                    .chain([r.scalar])
                    .any(|v| !v.is_finite())
        })
    {
        return Err(Error("invalid deformation impact input"));
    }
    let count = input.dimensions.iter().product::<usize>();
    let mut out = Output {
        nodes: input.nodes.to_vec(),
        lengths: input.edges.iter().map(|e| e.length).collect(),
        offset: input.offset,
        counter: input.counter,
        dirty: input.dirty,
    };
    if input.records.is_empty() {
        return Ok(out);
    }
    out.counter = input.iterations;
    let mut sum = [0.; 4];
    let mut active = 0;
    for n in out.nodes.iter().take(count).filter(|n| n.marker != 0.) {
        for (s, p) in sum.iter_mut().zip(n.position) {
            *s += p;
        }
        active += 1;
    }
    if active == 0 {
        return Ok(out);
    }
    let before_mean = mean(sum, active)?;
    let threshold_squared = input.displacement_threshold * input.displacement_threshold;
    let scalars: Vec<_> = input
        .records
        .iter()
        .map(|r| {
            let n = &out.nodes[r.node];
            let d: [f32; 3] = std::array::from_fn(|i| n.position[i] - n.rest[i]);
            let squared = (d[0] * d[0] + d[1] * d[1]) + d[2] * d[2];
            if threshold_squared > squared {
                r.scalar
            } else {
                r.scalar * input.supplied_gain
            }
        })
        .collect();
    if !scalars.iter().any(|v| *v > f32::from_bits(0x3a83126f)) {
        return Ok(out);
    }
    while out.counter > 0 {
        out.counter -= 1;
        for (record, scalar) in input.records.iter().zip(&scalars) {
            let n = &mut out.nodes[record.node];
            let delta: [f32; 4] = std::array::from_fn(|i| {
                if i == 3 {
                    0.
                } else {
                    n.position[i] - record.anchor[i]
                }
            });
            let products: [f32; 4] = std::array::from_fn(|i| delta[i] * record.direction[i]);
            let dot = (products[0] + products[1]) + (products[2] + products[3]);
            let step = 1. - out.counter.min(input.iterations - 1) as f32 / input.iterations as f32;
            let value = step * (*scalar - dot);
            let value = if *scalar > value { value } else { *scalar };
            if value > 0. {
                for (p, d) in n.position.iter_mut().zip(record.direction) {
                    *p = value * d + *p;
                }
            }
        }
        if input.clip_each_iteration {
            for n in out.nodes.iter_mut().take(count) {
                clip(n);
            }
        }
        // Original packs four edges and drops its trailing incomplete group.
        for edge in input.edges.iter().take(input.edges.len() / 4 * 4) {
            let [a, b] = edge.endpoints;
            let delta: [f32; 4] =
                std::array::from_fn(|i| out.nodes[b].position[i] - out.nodes[a].position[i]);
            let squared = (delta[0] * delta[0] + delta[1] * delta[1]) + delta[2] * delta[2];
            let inverse = reference_seed(squared, false)?;
            let distance = reference_seed(inverse, true)?;
            let factor = (edge.length - distance) * inverse;
            let correction = delta.map(|v| factor * v);
            // Both endpoint values are loaded before either write, including
            // aliased endpoints. Edge order then controls accumulated motion.
            let old_b = out.nodes[b].motion;
            for (i, c) in correction.into_iter().enumerate() {
                out.nodes[a].motion[i] -= c;
                out.nodes[b].motion[i] = old_b[i] + c;
            }
        }
        // Original unrolls four nodes, including padding outside dimensions.
        for n in out.nodes.iter_mut().take(count.div_ceil(4) * 4) {
            for i in 0..4 {
                let step = if i == 3 { 0. } else { input.integration_step };
                let change = n.motion[i] * step;
                n.position[i] += change;
                n.motion[i] *= input.damping;
            }
        }
    }
    sum = [0.; 4];
    for (n, previous) in out
        .nodes
        .iter_mut()
        .zip(input.nodes)
        .take(count)
        .filter(|(n, _)| n.marker != 0.)
    {
        for (i, s) in sum.iter_mut().enumerate() {
            *s = n.position[i] + *s;
            n.motion[i] = input.motion_scale * (n.position[i] - previous.position[i]);
        }
    }
    let after_mean = mean(sum, active)?;
    let removed: [f32; 4] = std::array::from_fn(|i| {
        if i == 3 {
            0.
        } else {
            after_mean[i] - before_mean[i]
        }
    });
    for n in out.nodes.iter_mut().take(count) {
        for (p, d) in n.position.iter_mut().zip(removed) {
            *p -= d;
        }
    }
    for (p, d) in out.offset.iter_mut().zip(removed) {
        *p -= d;
    }
    let settle_nodes: Vec<_> = out
        .nodes
        .iter()
        .map(|n| deformation_settle::Node {
            position: n.position,
            rest: n.rest,
            bounds: n.bounds,
        })
        .collect();
    let settled = deformation_settle::calculate(deformation_settle::Input {
        dimensions: input.dimensions,
        spacing: input.spacing,
        mode: input.mode,
        threshold: input.strain_threshold,
        gain: input.strain_gain,
        nodes: &settle_nodes,
        edges: input.edges,
    })?;
    for (n, position) in out.nodes.iter_mut().zip(settled.positions) {
        n.position = position;
    }
    out.lengths = settled.lengths;
    out.dirty = true;
    if out.offset.iter().any(|v| !v.is_finite())
        || out
            .nodes
            .iter()
            .take(used)
            .flat_map(|n| n.motion)
            .any(|v| !v.is_finite())
    {
        return Err(Error("nonfinite deformation impact output"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input<'a>(nodes: &'a [Node], records: &'a [Record]) -> Input<'a> {
        Input {
            dimensions: [2, 2, 2],
            spacing: 1.,
            mode: 0,
            strain_threshold: 0.,
            strain_gain: 20.,
            displacement_threshold: 0.05,
            supplied_gain: 1.,
            integration_step: 0.1,
            damping: 0.857,
            motion_scale: 1.,
            iterations: 10,
            counter: 3,
            clip_each_iteration: true,
            offset: [0.; 4],
            dirty: false,
            nodes,
            edges: &[],
            records,
        }
    }

    #[test]
    fn empty_and_below_gate_have_distinct_counter_side_effects() {
        let node = Node {
            position: [0., -0., 0., 1.],
            rest: [0., 0., 0., 1.],
            bounds: [1.; 4],
            motion: [0.25; 4],
            marker: 1.,
        };
        let nodes = vec![node; 400];
        let empty = calculate(input(&nodes, &[])).unwrap();
        let records = [Record {
            direction: [1., 0., 0., 0.],
            anchor: node.position,
            node: 0,
            scalar: f32::from_bits(0x3a83126f),
        }];
        let gated = calculate(input(&nodes, &records)).unwrap();
        assert_eq!((empty.counter, gated.counter), (3, 10));
        for out in [empty, gated] {
            assert!(!out.dirty);
            assert!(out.nodes.iter().all(|n| n.position.map(f32::to_bits)
                == node.position.map(f32::to_bits)
                && n.motion.map(f32::to_bits) == node.motion.map(f32::to_bits)));
        }
    }

    #[test]
    fn invalid_pool_rejected_before_empty_record_return() {
        let mut nodes = vec![
            Node {
                position: [0.; 4],
                rest: [0.; 4],
                bounds: [1.; 4],
                motion: [0.; 4],
                marker: 0.
            };
            400
        ];
        nodes[399].bounds[2] = -1.;
        assert!(calculate(input(&nodes, &[])).is_err());
        nodes[399].bounds[2] = 1.;
        nodes.pop();
        assert!(calculate(input(&nodes, &[])).is_err());
    }

    #[test]
    fn spare_node_padding_edges_do_not_fail_the_step() {
        // F401: a 3x3x3 grid (27 nodes, 28 with the unroll) whose packed edge
        // list pads its last group with edges on the spare node 399.
        let mut nodes = vec![
            Node {
                position: [0., 0., 0., 1.],
                rest: [0., 0., 0., 1.],
                bounds: [1.; 4],
                motion: [0.; 4],
                marker: 1.,
            };
            400
        ];
        for (i, n) in nodes.iter_mut().enumerate().take(27) {
            let p = [(i % 3) as f32, (i / 3 % 3) as f32, (i / 9) as f32, 1.];
            n.position = p;
            n.rest = p;
        }
        let edge = |a, b| deformation_settle::Edge {
            endpoints: [a, b],
            length: 1.,
        };
        let edges = [edge(0, 1), edge(1, 2), edge(399, 399), edge(399, 399)];
        let records = [Record {
            direction: [1., 0., 0., 0.],
            anchor: [-0.5, 0., 0., 1.],
            node: 0,
            scalar: 0.2,
        }];
        let mut step = input(&nodes, &records);
        step.dimensions = [3, 3, 3];
        step.edges = &edges;
        let out = calculate(step).unwrap();
        assert!(out.nodes[..28].iter().all(|n| n.motion.iter().all(|v| v.is_finite())));
        assert!(!out.nodes[399].motion.iter().all(|v| v.is_finite()));
        // The spare node's motion is carried into the next step.
        let mut next = input(&out.nodes, &records);
        next.dimensions = [3, 3, 3];
        next.edges = &edges;
        assert!(calculate(next).is_ok());
    }
}
