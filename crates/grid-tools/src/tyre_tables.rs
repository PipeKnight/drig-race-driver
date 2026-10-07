//! Source CSV joined to the verified material speed grip and key cache.
use grid_world::tyre_grip::{self, Key};

pub struct Tables(grid_formats::tyre_table::Table);
impl Tables {
    pub fn read(bytes: &[u8]) -> Result<Self, grid_formats::Error> {
        Ok(Self(grid_formats::tyre_table::read(bytes)?))
    }
    pub fn grip(
        &self,
        row: usize,
        key: &mut Key,
        coefficients: [f32; 3],
        speed: f32,
    ) -> Result<f32, grid_world::Error> {
        let values = self
            .0
            .rows
            .get(row)
            .ok_or(grid_world::Error("invalid tyre row slot"))?;
        let factor = tyre_grip::factor(
            &self.0.columns,
            values.as_deref().unwrap_or(&[]),
            self.0.active(),
            key,
        )?;
        tyre_grip::grip(
            coefficients[0],
            coefficients[1],
            coefficients[2],
            speed,
            factor,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_rows_only_fail_when_the_lookup_needs_their_values() {
        let tables = Tables::read(b",,TEST\r\nlabels\r\nLIGR,,100\r\n").unwrap();
        let mut absent = Key::Code(*b"MISS");
        assert_eq!(tables.grip(1, &mut absent, [10., 1., 2.], 4.).unwrap(), 12.);
        assert_eq!(absent, Key::Code(*b"MISS"));
        let mut present = Key::Code(*b"TEST");
        assert!(tables.grip(1, &mut present, [10., 1., 2.], 4.).is_err());
        assert_eq!(present, Key::Index(0));
        assert_eq!(
            tables.grip(0, &mut present, [10., 1., 2.], 4.).unwrap(),
            12.
        );
    }
}
