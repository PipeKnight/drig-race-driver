//! Read-only readers for verified GRID framing and selected mesh/texture subsets.
//! Unknown values remain bytes; scene assembly and game behavior are not inferred.

pub mod action_map;
pub mod ai;
pub mod audio_car_setup;
pub mod audio_dictionary;
pub mod binxml;
pub mod car_collision;
pub mod collision;
pub mod cqtc;
pub mod damage_boxes;
pub mod deformation;
pub mod entities;
pub mod etc2;
pub mod graphics_grade;
pub mod grid;
pub mod ground_cover;
pub mod handling_damage;
mod handling_damage_fields;
pub mod jpk;
pub mod kickup;
pub mod language;
pub mod material;
pub mod mesh;
pub mod ornaments;
pub mod tree_placements;
pub mod physparams;
pub mod progress;
pub mod pssg;
pub mod scene;
pub mod skidmarks;
pub mod start_lights;
pub mod surface;
pub mod texture;
pub mod tuning;
pub mod tyre_table;
pub mod vehicle;
pub mod vehicle_properties;
pub mod vehicle_views;
mod vehicle_properties_fields;

use std::{fmt, ops::Range};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub offset: usize,
    pub message: &'static str,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at 0x{:x}: {}", self.offset, self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

fn fail<T>(offset: usize, message: &'static str) -> Result<T> {
    Err(Error { offset, message })
}

fn range(start: usize, length: usize, end: usize) -> Result<Range<usize>> {
    match start.checked_add(length) {
        Some(stop) if start <= end && stop <= end => Ok(start..stop),
        _ => fail(start, "range exceeds enclosing boundary"),
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let r = range(self.pos, length, self.bytes.len())?;
        self.pos = r.end;
        Ok(&self.bytes[r])
    }
    fn be(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn le(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn name(&mut self) -> Result<&'a [u8]> {
        let length = self.be()? as usize;
        self.take(length)
    }
}

pub mod track_lighting;
