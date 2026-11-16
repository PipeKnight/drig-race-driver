//! Correct perspective reprojection in Bevy's depth-zero motion prepass.
use bevy::{asset::io::embedded::EmbeddedAssetRegistry, prelude::App, render::RenderApp};
use std::path::{Path, PathBuf};

pub(crate) fn install(app: &App) {
    if app.get_sub_app(RenderApp).is_none() {
        return;
    }

    // Bevy 0.19.1 embeds this shader during plugin build and loads it in
    // RenderStartup. Keep the existing asset key and binding/output layout.
    app.world().resource::<EmbeddedAssetRegistry>().insert_asset(
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/sky_motion_projected.wgsl"
        )),
        Path::new("bevy_core_pipeline/prepass/background_motion_vectors.wgsl"),
        include_bytes!("sky_motion_projected.wgsl"),
    );
}
