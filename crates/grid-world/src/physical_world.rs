//! Selected whole serial world step and callback-free wrapper consumption.
//! Cold all-active owners; no registered pose cursor, body shapes, broadphase
//! pairs, joints, sleeping/wakeup, callbacks, allocation or asynchronous workers.
use crate::{Error, contact_groups, grouped_physical_frame};

pub fn calculate_cold(
    input: &contact_groups::Input,
    manager: &grouped_physical_frame::Manager,
    shapes: &[contact_groups::Shape],
) -> Result<grouped_physical_frame::Output, Error> {
    if input.bodies.iter().any(|b| (b[0x414 / 4] >> 8) & 255 != 0) {
        return Err(Error("unsupported deferred world pose publication"));
    }
    // In this owned scope the whole original scheduler's geometry queues are
    // empty. Its advance/group/solve stages are the verified integrated path.
    let mut out = grouped_physical_frame::calculate_tick_with_shapes(input, manager, shapes)?;
    // Original A727A0 consumes the transient pool after callback dispatch. Both
    // owned shape callback slots21C are absent; record bytes remain for reuse.
    out.pool_used = 0;
    Ok(out)
}
