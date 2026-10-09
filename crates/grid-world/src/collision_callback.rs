//! Supplied contacts through the complete registered collision callback.
use crate::{
    Error, contact_inverse, contact_preparation, contact_reduction, deformation_settle,
    impact_producer,
};

pub struct Input<'a> {
    pub producer: impact_producer::Input<'a>,
    pub intensity: f32,
    pub descriptor_disabled: bool,
    pub global_disabled: bool,
    pub policy: contact_preparation::Policy,
}
pub struct Activation {
    pub strength: f32,
    pub point: [f32; 4],
    pub direction: [f32; 4],
}
pub struct Output {
    /// Absent when the callback's count/disable guard returns false.
    pub producer: Option<impact_producer::Output>,
    pub activation: Option<Activation>,
}
pub fn calculate(input: Input<'_>) -> Result<Output, Error> {
    let mut p = input.producer;
    if p.generation.contacts.is_empty() || input.descriptor_disabled || input.global_disabled {
        return Ok(Output {
            producer: None,
            activation: None,
        });
    }
    let prepared = contact_reduction::reduce(p.generation.contacts)?;
    let inverse = contact_inverse::calculate(p.generation.pose)?;
    let settle_nodes: Vec<_> = p
        .grid
        .nodes
        .iter()
        .map(|n| deformation_settle::Node {
            position: n.position,
            rest: n.rest,
            bounds: n.bounds,
        })
        .collect();
    let preparation = contact_preparation::calculate(contact_preparation::Input {
        grid: deformation_settle::Input {
            dimensions: p.grid.dimensions,
            spacing: p.grid.spacing,
            mode: p.grid.mode,
            threshold: p.grid.strain_threshold,
            gain: p.grid.strain_gain,
            nodes: &settle_nodes,
            edges: p.grid.edges,
        },
        intensity: input.intensity,
        disabled: input.descriptor_disabled,
        up: p.generation.pose[1],
        direction: prepared.direction,
        point: prepared.point,
        inverse,
        policy: input.policy,
    })?;
    // Preparation preserves these world-space operands; body adjustment receives them.
    p.body.point = prepared.point;
    p.body.direction = prepared.direction;
    let mut nodes = p.grid.nodes.to_vec();
    let mut edges = p.grid.edges.to_vec();
    let activation = if let Some(active) = preparation.activated {
        for (node, position) in nodes.iter_mut().zip(active.settled.positions) {
            node.position = position;
        }
        for (edge, length) in edges.iter_mut().zip(active.settled.lengths) {
            edge.length = length;
        }
        p.grid.mode = 1;
        p.grid.dirty = [1; 5];
        Some(Activation {
            strength: active.strength,
            point: active.point,
            direction: active.direction,
        })
    } else {
        None
    };
    let result = impact_producer::calculate(impact_producer::Input {
        grid: impact_producer::Grid {
            nodes: &nodes,
            edges: &edges,
            ..p.grid
        },
        generation: p.generation,
        body: p.body,
        response: p.response,
    })?;
    Ok(Output {
        producer: Some(result),
        activation,
    })
}
