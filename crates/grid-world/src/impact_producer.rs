//! Supplied contacts through generation, body adjustment, grid solving and response.
//! Contact detection, initialization and tick/random scheduling remain external.
use crate::{
    Error, deformation_impact as solver, deformation_settle::Edge, impact_body as body,
    impact_generation as generation, impact_response as response,
};

pub struct Grid<'a> {
    pub dimensions: [usize; 3],
    pub spacing: f32,
    pub minimum: [f32; 4],
    pub state: f32,
    pub corners: [f32; 4],
    pub window: [f32; 10],
    pub flags: &'a [u8],
    pub dirty: [u8; 5],
    pub nodes: &'a [solver::Node],
    pub edges: &'a [Edge],
    pub mode: u32,
    pub strain_threshold: f32,
    pub strain_gain: f32,
    pub displacement_threshold: f32,
    pub integration_step: f32,
    pub damping: f32,
    pub motion_scale: f32,
    pub iterations: u32,
    pub counter: u32,
    pub clip_each_iteration: bool,
    pub offset: [f32; 4],
}
pub struct Generation<'a> {
    pub pose: [[f32; 4]; 4],
    pub body_offset: [f32; 4],
    pub body_b20: [f32; 4],
    pub velocity: [f32; 4],
    pub supplied_scalar: f32,
    pub corner_scale: f32,
    pub static_mass: f32,
    pub limit_scale: f32,
    pub flag_radius_scale: f32,
    pub record_radius_scale: f32,
    pub radial_scale: f32,
    pub damage_threshold: f32,
    pub damage_gain: f32,
    pub projection: f32,
    pub contacts: &'a [generation::Contact],
}
pub struct Body {
    pub angular: [f32; 4],
    pub linear: [f32; 4],
    pub point: [f32; 4],
    pub direction: [f32; 4],
    pub center: [f32; 4],
    pub history: [f32; 10],
    pub history_extra: f32,
    pub mass: f32,
    pub gain: f32,
    pub random_gain: f32,
    pub history_range: f32,
    pub history_threshold: f32,
    pub speed_threshold: f32,
    pub random_seed: u32,
}
pub struct Response<'a> {
    pub values: Option<&'a [f32]>,
    pub accumulator: [f32; 4],
    pub dirty: bool,
    pub scale: f32,
    pub correction_gain: f32,
    pub subtract_baseline: bool,
}
pub struct Input<'a> {
    pub grid: Grid<'a>,
    pub generation: Generation<'a>,
    pub body: Body,
    pub response: Response<'a>,
}
pub struct Output {
    pub generation: generation::Output,
    pub body: body::Output,
    pub grid: solver::Output,
    pub response: response::Output,
}

pub fn calculate(input: Input<'_>) -> Result<Output, Error> {
    let g = input.grid;
    let c = input.generation;
    let b = input.body;
    let r = input.response;
    let nodes: Vec<_> = g
        .nodes
        .iter()
        .map(|n| generation::Node {
            position: n.position,
            marker: n.marker,
        })
        .collect();
    let generated = generation::calculate(generation::Input {
        dimensions: g.dimensions,
        spacing: g.spacing,
        minimum: g.minimum,
        pose: c.pose,
        body_offset: c.body_offset,
        body_b20: c.body_b20,
        velocity: c.velocity,
        supplied_scalar: c.supplied_scalar,
        state: g.state,
        corners: g.corners,
        window: g.window,
        corner_scale: c.corner_scale,
        static_mass: c.static_mass,
        limit_scale: c.limit_scale,
        flag_radius_scale: c.flag_radius_scale,
        record_radius_scale: c.record_radius_scale,
        radial_scale: c.radial_scale,
        damage_threshold: c.damage_threshold,
        damage_gain: c.damage_gain,
        projection: c.projection,
        flags: g.flags,
        dirty: g.dirty,
        nodes: &nodes,
        contacts: c.contacts,
    })?;
    let body_input = body::Input {
        angular: b.angular,
        linear: b.linear,
        frame: generation::contact_frame(c.pose, c.body_offset)?,
        point: b.point,
        direction: b.direction,
        center: b.center,
        history: b.history,
        history_extra: b.history_extra,
        mass: b.mass,
        gain: b.gain,
        random_gain: b.random_gain,
        history_range: b.history_range,
        history_threshold: b.history_threshold,
        speed_threshold: b.speed_threshold,
        random_seed: b.random_seed,
    };
    // Validate the bounded API even when the original skips this branch.
    body::validate(&body_input)?;
    // The whole original routine bypasses body adjustment when no records exist.
    let adjusted = if generated.records.is_empty() {
        body::Output {
            angular: b.angular,
            linear: b.linear,
            history: b.history,
            history_extra: b.history_extra,
            random_seed: b.random_seed,
        }
    } else {
        body::calculate(body_input)?
    };
    let records: Vec<_> = generated
        .records
        .iter()
        .map(|r| solver::Record {
            direction: r.direction,
            anchor: r.anchor,
            node: usize::from(r.node),
            scalar: r.scalar,
        })
        .collect();
    let solved = solver::calculate(solver::Input {
        dimensions: g.dimensions,
        spacing: g.spacing,
        mode: g.mode,
        strain_threshold: g.strain_threshold,
        strain_gain: g.strain_gain,
        displacement_threshold: g.displacement_threshold,
        supplied_gain: 1.,
        integration_step: g.integration_step,
        damping: g.damping,
        motion_scale: g.motion_scale,
        iterations: g.iterations,
        counter: g.counter,
        clip_each_iteration: g.clip_each_iteration,
        offset: g.offset,
        dirty: generated.dirty[0] != 0,
        nodes: g.nodes,
        edges: g.edges,
        records: &records,
    })?;
    let nodes: Vec<_> = solved.nodes.iter().map(|n| n.position).collect();
    let records: Vec<_> = generated
        .records
        .iter()
        .map(|r| response::Record {
            direction: r.direction,
            anchor: r.anchor,
            node: r.node,
            contact: r.contact,
            weight: r.weight,
        })
        .collect();
    let contacts: Vec<_> = c
        .contacts
        .iter()
        .map(|c| response::Contact {
            enabled: c.enabled,
            reverse: c.reverse,
            baseline: c.value_38,
            positive_gate: c.value_3c,
            normal: c.direction,
        })
        .collect();
    let responded = response::calculate(response::Input {
        nodes: &nodes,
        records: &records,
        contacts: &contacts,
        response: r.values,
        normalization_vector: c.velocity,
        accumulator: r.accumulator,
        dirty: r.dirty,
        scale: r.scale,
        correction_gain: r.correction_gain,
        subtract_baseline: r.subtract_baseline,
    })?;
    Ok(Output {
        generation: generated,
        body: adjusted,
        grid: solved,
        response: responded,
    })
}
