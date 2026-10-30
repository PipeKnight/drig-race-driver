//! Resource-to-behavior adapters shared by local diagnostics.
pub mod ambience_audio;
pub mod audio_mix;
pub mod audio_pacing;
pub mod ai_driver;
pub mod ai_pacing;
pub mod body_deformation;
pub mod car;
pub mod collision_audio;
pub mod damage_audio;
pub mod career;
pub mod body_geometry;
pub mod body_impact_parameters;
pub mod body_impacts;
pub mod contact_diagnostic;
pub mod contact_surfaces;
pub mod cqtc_geometry;
pub mod deformation_parameters;
pub mod drift;
pub mod dynamic_props;
pub mod engine_audio;
pub mod environment;
pub mod engine_audio_continuous;
pub mod engine_audio_control;
pub mod engine_audio_gain;
pub mod engine_audio_publication;
pub mod engine_audio_restart;
pub mod engine_audio_schedule;
pub mod engine_audio_load;
pub mod engine_audio_shift;
pub mod engine_audio_whine;
pub mod mech_audio;
pub mod hud_audio;
pub mod entity_geometry;
pub mod flashback;
pub mod impact_diagnostic;
pub mod install;
pub mod lap_progress;
pub mod parallel;
pub mod perf;
pub mod loose_panel;
pub mod loose_parts;
pub mod loose_wheel;
pub mod private_parameters;
pub mod pursuit_driver;
pub mod race;
pub mod route;
pub mod series;
pub mod source_deformation;
pub mod source_physical;
pub mod soak;
pub mod soak_perf;
pub mod source_vehicle;
pub mod speech;
pub mod time_of_day;
pub mod standard_car;
pub mod start_lights;
mod standard_car_fields;
pub mod time_trial;
pub mod touge;
pub mod demolition;
pub mod track;
pub mod tyre_audio;
pub mod tyre_smoke;
pub mod tyre_tables;
pub mod vehicle_world_diagnostic;
pub mod virtual_race;

/// The installation's NDBS database (`database/schema.bin` with
/// `database/database.bin`). A missing file is named in the error: a bare
/// `os error 2` was all a bundled APK without `database/` showed (F539).
pub fn read_database(
    root: &std::path::Path,
) -> Result<grid_formats::database::Database, Box<dyn std::error::Error>> {
    let file = |name: &str| {
        std::fs::read(root.join(name)).map_err(|e| format!("{name}: {e}"))
    };
    Ok(grid_formats::database::read(&file("database/schema.bin")?, &file("database/database.bin")?)?)
}
