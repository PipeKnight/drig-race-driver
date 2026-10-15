//! Authored material responses, not direct mappings of GRID shader constants.
use bevy::prelude::*;

pub fn apply(name: &str, material: &mut StandardMaterial) {
    match name {
        "Car Body Paint" => {
            material.perceptual_roughness = 0.3;
            material.metallic = 0.0;
            material.clearcoat = 1.0;
            material.clearcoat_perceptual_roughness = 0.12;
        }
        "Window Glass" | "Head Lights Glass" | "Rear Lights Glass" => {
            material.perceptual_roughness = 0.08;
            material.metallic = 0.0;
            material.ior = 1.5;
        }
        "Tyre Wall" | "Tyre Tread" => {
            material.perceptual_roughness = if name == "Tyre Tread" { 0.9 } else { 0.82 };
            material.reflectance = 0.3;
            material.metallic = 0.0;
        }
        "Wheel" => {
            // White painted alloy wheel: keep a dielectric coating rather than
            // treating all of its diffuse livery as bare reflective metal.
            material.perceptual_roughness = 0.32;
            material.metallic = 0.15;
            material.clearcoat = 0.3;
            material.clearcoat_perceptual_roughness = 0.2;
        }
        "Bare Metal" | "Shiny Metal" | "Discs" => {
            material.metallic = 0.95;
            material.perceptual_roughness = match name {
                "Shiny Metal" => 0.22,
                "Discs" => 0.48,
                _ => 0.35,
            };
        }
        "Cabin" | "Matt" | "Aerials" => {
            material.metallic = 0.0;
            material.perceptual_roughness = 0.85;
        }
        _ => {}
    }
}
