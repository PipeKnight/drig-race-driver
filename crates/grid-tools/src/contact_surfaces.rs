//! Ordered source material keys joined to the verified wheel-contact fields.
use grid_formats::{binxml, surface};
use grid_world::contact_material;

pub struct Surfaces {
    keys: Vec<[u8; 4]>,
    mechanics: Vec<Option<contact_material::Material>>,
}

impl Surfaces {
    pub fn read(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("surface source exceeds bound".into());
        }
        let document = binxml::read(bytes)?;
        let materials = surface::read(&document)?;
        let keys = materials.iter().map(|m| m.code).collect();
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
        Ok(Self { keys, mechanics })
    }

    pub fn contact(
        &self,
        code: [u8; 4],
    ) -> Result<(usize, contact_material::Material), Box<dyn std::error::Error>> {
        let index = surface::packed_index(&self.keys, code, true).ok_or("surface table absent")?;
        let mechanics = self.mechanics[index].ok_or("selected surface has no mechanics")?;
        Ok((index, mechanics))
    }
}
