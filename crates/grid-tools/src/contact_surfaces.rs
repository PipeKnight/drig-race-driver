//! Ordered source material keys joined to the verified wheel-contact fields.
use grid_formats::{binxml, surface};
use grid_world::{contact_material, surface_drag, wheel_history};

pub struct Surfaces {
    keys: Vec<[u8; 4]>,
    mechanics: Vec<Option<contact_material::Material>>,
    drag: Vec<Option<surface_drag::Material>>,
    history: Vec<Option<wheel_history::Material>>,
    joined: Vec<Option<grid_world::vehicle_contact_force::Material>>,
    physical: Vec<grid_world::contact_rows::Material>,
    sheets: Vec<bool>,
}

impl Surfaces {
    pub fn read(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("surface source exceeds bound".into());
        }
        let document = binxml::read(bytes)?;
        let materials = surface::read(&document)?;
        let keys = materials.iter().map(|m| m.code).collect();
        let sheets = materials.iter().map(|m| m.physical.sheet).collect();
        let physical = materials
            .iter()
            .map(|m| m.physical.row_words(m.code).map(f32::from_bits))
            .collect();
        let mechanics = materials
            .iter()
            .map(|m| {
                m.mechanics.as_ref().map(|s| contact_material::Material {
                    time_based: s.time_based,
                    wavelength: s.wavelength,
                    magnitude: s.magnitude,
                    real_proportion: s.real_proportion,
                    car_proportion: s.car_proportion,
                    height_offset: s.depth,
                })
            })
            .collect();
        let drag: Vec<_> = materials
            .iter()
            .map(|m| {
                m.mechanics.as_ref().map(|s| surface_drag::Material {
                    minimum_speed: s.minimum_speed_for_slowdown,
                    maximum_speed: s.speed_for_maximum_slowdown,
                    slowdown: s.slowdown,
                    // Original source initializer and loader clear +73.
                    flag_73: false,
                })
            })
            .collect();
        let history: Vec<_> = materials
            .iter()
            .enumerate()
            .map(|(i, m)| {
                m.mechanics.as_ref().map(|s| wheel_history::Material {
                    wear: s.wear,
                    speed_grip: [s.grip_per_ms, s.ms_for_max_effect, s.grip],
                    flag_70: true,
                    dirt_build_up: s.dirt_build_up,
                    token: i as u32 + 1,
                })
            })
            .collect();
        let joined = materials
            .iter()
            .enumerate()
            .map(|(i, m)| {
                m.mechanics
                    .as_ref()
                    .map(|s| grid_world::vehicle_contact_force::Material {
                        history: history[i].unwrap(),
                        prepared: grid_world::vehicle_prepared_frame::Material {
                            drag: drag[i].unwrap(),
                            roughness: s.roughness,
                            roll: s.roll_induce,
                        },
                        hardness: s.hardness,
                        high_downforce: s.high_downforce,
                    })
            })
            .collect();
        Ok(Self {
            keys,
            mechanics,
            drag,
            history,
            joined,
            physical,
            sheets,
        })
    }

    /// Select by the source packed-code policy; this does not establish which
    /// resource key a live car shape owns.
    pub fn physical(
        &self,
        code: [u8; 4],
    ) -> Result<(usize, grid_world::contact_rows::Material), Box<dyn std::error::Error>> {
        let index = surface::packed_index(&self.keys, code, true).ok_or("surface table absent")?;
        Ok((index, self.physical[index]))
    }

    /// Exact four-byte source lookup with first-record fallback, matching the
    /// selected packed constructor policy. No key padding normalization.
    pub fn sheet(&self, code: [u8; 4]) -> Result<bool, grid_world::Error> {
        let index = surface::packed_index(&self.keys, code, true)
            .ok_or(grid_world::Error("surface table absent"))?;
        Ok(self.sheets[index])
    }

    pub fn joined(
        &self,
        token: u32,
    ) -> Result<Option<grid_world::vehicle_contact_force::Material>, grid_world::Error> {
        if token == 0 {
            return Ok(None);
        }
        self.joined
            .get(token as usize - 1)
            .copied()
            .flatten()
            .map(Some)
            .ok_or(grid_world::Error("unknown source mechanics token"))
    }

    pub fn history(
        &self,
        token: u32,
    ) -> Result<Option<wheel_history::Material>, grid_world::Error> {
        if token == 0 {
            return Ok(None);
        }
        self.history
            .get(token as usize - 1)
            .copied()
            .flatten()
            .map(Some)
            .ok_or(grid_world::Error("unknown source mechanics token"))
    }

    pub fn contact(
        &self,
        code: [u8; 4],
    ) -> Result<(usize, contact_material::Material), Box<dyn std::error::Error>> {
        let index = surface::packed_index(&self.keys, code, true).ok_or("surface table absent")?;
        let mechanics = self.mechanics[index].ok_or("selected surface has no mechanics")?;
        Ok((index, mechanics))
    }

    pub fn drag(
        &self,
        code: [u8; 4],
    ) -> Result<(usize, surface_drag::Material), Box<dyn std::error::Error>> {
        let index = surface::packed_index(&self.keys, code, true).ok_or("surface table absent")?;
        let material = self.drag[index].ok_or("selected surface has no mechanics")?;
        Ok((index, material))
    }
}
