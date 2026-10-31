//! Ordered source material keys joined to the verified wheel-contact fields.
use grid_formats::{binxml, surface};
use grid_world::{contact_material, surface_drag, wheel_history};

#[derive(Clone)]
pub struct Surfaces {
    keys: Vec<[u8; 4]>,
    mechanics: Vec<contact_material::Material>,
    drag: Vec<surface_drag::Material>,
    history: Vec<wheel_history::Material>,
    joined: Vec<grid_world::vehicle_contact_force::Material>,
    physical: Vec<grid_world::contact_rows::Material>,
    sheets: Vec<bool>,
    names: Vec<Option<String>>,
}

impl Surfaces {
    pub fn read(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("surface source exceeds bound".into());
        }
        let document = binxml::read(bytes)?;
        let materials = surface::read(&document)?;
        let keys = materials.iter().map(|m| m.code).collect();
        let names = materials
            .iter()
            .map(|m| m.mechanics.as_ref().map(|s| s.name.to_owned()))
            .collect();
        // A material without MECHANICS (WARN, RESE, RESD, UNRE, FBND) keeps
        // the initializer's record; the original wheel contact reads it as is.
        let records: Vec<surface::Mechanics<'_>> = materials
            .iter()
            .map(|m| m.mechanics.clone().unwrap_or_else(surface::Mechanics::initial))
            .collect();
        let sheets = materials.iter().map(|m| m.physical.sheet).collect();
        let physical = materials
            .iter()
            .map(|m| m.physical.row_words(m.code).map(f32::from_bits))
            .collect();
        let mechanics = records
            .iter()
            .map(|s| contact_material::Material {
                time_based: s.time_based,
                wavelength: s.wavelength,
                magnitude: s.magnitude,
                real_proportion: s.real_proportion,
                car_proportion: s.car_proportion,
                height_offset: s.depth,
            })
            .collect();
        let drag: Vec<_> = records
            .iter()
            .map(|s| surface_drag::Material {
                minimum_speed: s.minimum_speed_for_slowdown,
                maximum_speed: s.speed_for_maximum_slowdown,
                slowdown: s.slowdown,
                // Original source initializer and loader clear +73.
                flag_73: false,
            })
            .collect();
        let history: Vec<_> = records
            .iter()
            .enumerate()
            .map(|(i, s)| wheel_history::Material {
                wear: s.wear,
                speed_grip: [s.grip_per_ms, s.ms_for_max_effect, s.grip],
                flag_70: true,
                dirt_build_up: s.dirt_build_up,
                token: i as u32 + 1,
            })
            .collect();
        let joined = records
            .iter()
            .enumerate()
            .map(|(i, s)| grid_world::vehicle_contact_force::Material {
                history: history[i],
                prepared: grid_world::vehicle_prepared_frame::Material {
                    drag: drag[i],
                    roughness: s.roughness,
                    roll: s.roll_induce,
                },
                hardness: s.hardness,
                high_downforce: s.high_downforce,
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
            names,
        })
    }

    /// Mechanics record name for a wheel contact token (wheel+C0, index+1).
    /// Original 4361C0 matches this name exactly against AudioMaterial names.
    pub fn mechanics_name(&self, token: u32) -> Option<&str> {
        self.names.get((token as usize).checked_sub(1)?)?.as_deref()
    }

    /// Four-byte material code for a wheel contact token (wheel+C0, index+1).
    pub fn code(&self, token: u32) -> Option<[u8; 4]> {
        self.keys.get((token as usize).checked_sub(1)?).copied()
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
            .map(Some)
            .ok_or(grid_world::Error("unknown source mechanics token"))
    }

    pub fn contact(
        &self,
        code: [u8; 4],
    ) -> Result<(usize, contact_material::Material), Box<dyn std::error::Error>> {
        let index = surface::packed_index(&self.keys, code, true).ok_or("surface table absent")?;
        Ok((index, self.mechanics[index]))
    }

    pub fn drag(
        &self,
        code: [u8; 4],
    ) -> Result<(usize, surface_drag::Material), Box<dyn std::error::Error>> {
        let index = surface::packed_index(&self.keys, code, true).ok_or("surface table absent")?;
        Ok((index, self.drag[index]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_materials_without_mechanics_read_the_initializer_record() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let s = Surfaces::read(&std::fs::read(root.join("surface_materials.xml")).unwrap()).unwrap();
        for code in [*b"WARN", *b"RESE", *b"RESD", *b"UNRE", *b"FBND"] {
            let (index, m) = s.contact(code).unwrap();
            assert_eq!(s.code(index as u32 + 1), Some(code));
            assert_eq!(s.mechanics_name(index as u32 + 1), None);
            assert!(m.time_based);
            assert_eq!(m.wavelength, f32::from_bits(0x3d4c_cccd));
            assert_eq!((m.magnitude, m.real_proportion, m.car_proportion), (0., 0., 0.));
            assert_eq!(m.height_offset, 0.);
            let (_, drag) = s.drag(code).unwrap();
            assert_eq!((drag.minimum_speed, drag.maximum_speed, drag.slowdown), (20., 40., 0.));
            let joined = s.joined(index as u32 + 1).unwrap().unwrap();
            assert_eq!(joined.hardness, 1.);
            assert_eq!(joined.history.speed_grip, [0., 1., 1.]);
        }
    }
}
