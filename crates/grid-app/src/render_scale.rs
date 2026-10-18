//! Render-height cap (`[video] render_height`, docs/SETTINGS.md): the 3D
//! view renders into an image of at most that many rows, and a second camera
//! stretches it over the window. Independently authored; GRID's PC renderer
//! always renders at the window size. On a phone, above Full HD costs GPU
//! time without a visible gain (owner, 2026-10-18), so Android defaults to
//! 1080 rows. The HUD, pause menu and touch zones are UI on the 3D camera, so
//! they are drawn into the same image and scaled with it (retargeting every
//! HUD module to the window camera was not a small change).
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::window::PrimaryWindow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderHeight {
    Native,
    Rows(u32),
}

impl RenderHeight {
    pub fn parse(value: &str) -> Result<Self, String> {
        if value.eq_ignore_ascii_case("native") {
            return Ok(Self::Native);
        }
        match value.parse::<u32>() {
            Ok(rows) if (240..=4320).contains(&rows) => Ok(Self::Rows(rows)),
            _ => Err("`render_height` needs native or 240..4320".into()),
        }
    }

    /// Android: 1080 rows; desktop: native.
    pub fn platform_default() -> Self {
        if cfg!(target_os = "android") {
            Self::Rows(1080)
        } else {
            Self::Native
        }
    }
}

/// `[video] present_mode`: only modes with a fallback chain, so an
/// unsupported choice cannot fail at swapchain set-up. Default `fifo` (Bevy's
/// default, strict vsync).
pub fn present_mode(value: &str) -> Result<bevy::window::PresentMode, String> {
    use bevy::window::PresentMode;
    match value.to_ascii_lowercase().as_str() {
        "fifo" => Ok(PresentMode::Fifo),
        "auto_vsync" => Ok(PresentMode::AutoVsync),
        "auto_no_vsync" => Ok(PresentMode::AutoNoVsync),
        _ => Err("`present_mode` needs fifo, auto_vsync or auto_no_vsync".into()),
    }
}

/// `[video] refresh_hz`: `system` (`None`, the default) or 30..240 Hz, a hint
/// for the display rate (Android `ANativeWindow_setFrameRate`).
pub fn refresh_hz(value: &str) -> Result<Option<f32>, String> {
    if value.eq_ignore_ascii_case("system") {
        return Ok(None);
    }
    match value.parse::<f32>() {
        Ok(hz) if (30. ..=240.).contains(&hz) => Ok(Some(hz)),
        _ => Err("`refresh_hz` needs system or 30..240".into()),
    }
}

/// `[video] frame_latency`: 1..3 frames in flight (`None`: wgpu's default 2).
pub fn frame_latency(value: &str) -> Result<std::num::NonZero<u32>, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|n| (1..=3).contains(n))
        .and_then(std::num::NonZero::new)
        .ok_or_else(|| "`frame_latency` needs 1..3".into())
}

/// Render size for a window of `physical` pixels: `None` renders straight to
/// the window (native, or a cap at or above the window height, so never an
/// upscale beyond native). The width keeps the window's aspect ratio.
pub fn scaled_size(physical: UVec2, height: RenderHeight) -> Option<UVec2> {
    let RenderHeight::Rows(rows) = height else {
        return None;
    };
    if physical.x == 0 || physical.y == 0 || rows >= physical.y {
        return None;
    }
    let width = (u64::from(physical.x) * u64::from(rows) + u64::from(physical.y) / 2)
        / u64::from(physical.y);
    Some(UVec2::new(width.max(1) as u32, rows))
}

/// The setting and the sizes in use, read by the frame-rate counter.
#[derive(Resource, Debug)]
pub struct RenderScale {
    pub height: RenderHeight,
    /// Window size in physical pixels.
    pub screen: UVec2,
    /// Size the 3D view renders at (equal to `screen` when native).
    pub render: UVec2,
    image: Option<Handle<Image>>,
    display: Option<Entity>,
}

/// Window camera that shows the scaled image.
#[derive(Component)]
struct DisplayCamera;

#[derive(Component)]
struct DisplayImage;

#[allow(clippy::too_many_arguments)]
fn apply(
    mut commands: Commands,
    mut scale: ResMut<RenderScale>,
    window: Query<&Window, With<PrimaryWindow>>,
    viewer: Query<Entity, With<crate::ViewerCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut shown: Query<&mut ImageNode, With<DisplayImage>>,
    touch: Option<Res<crate::touch_controls::Mobile>>,
    mut ui_scale: ResMut<UiScale>,
) {
    let (Some(w), Some(camera)) = (window.iter().next(), viewer.iter().next()) else {
        return;
    };
    let screen = UVec2::new(w.physical_width(), w.physical_height());
    if screen == scale.screen && (scale.image.is_some() || scale.render == screen) {
        return;
    }
    scale.screen = screen;
    let Some(size) = scaled_size(screen, scale.height) else {
        // Native: the 3D camera draws to the window as before.
        if scale.image.take().is_some() {
            commands.entity(camera).insert(RenderTarget::default());
            if let Some(display) = scale.display.take() {
                commands.entity(display).despawn();
            }
        }
        scale.render = screen;
        if touch.is_none() {
            ui_scale.0 = 1.;
        }
        println!(
            "render_scale height={:?} render={}x{} screen={}x{} mode=native",
            scale.height, screen.x, screen.y, screen.x, screen.y
        );
        return;
    };
    let image = images.add(Image::new_target_texture(
        size.x,
        size.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands
        .entity(camera)
        .insert((RenderTarget::Image(image.clone().into()), IsDefaultUiCamera));
    match scale.display {
        Some(_) => {
            for mut node in &mut shown {
                node.image = image.clone();
            }
        }
        None => {
            let display = commands
                .spawn((
                    DisplayCamera,
                    Camera2d,
                    Camera {
                        order: 1,
                        ..default()
                    },
                    Msaa::Off,
                ))
                .id();
            commands.spawn((
                DisplayImage,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100.),
                    height: percent(100.),
                    ..default()
                },
                ImageNode::new(image.clone()),
                UiTargetCamera(display),
            ));
            scale.display = Some(display);
        }
    }
    scale.image = Some(image);
    scale.render = size;
    // Without touch mode (which sets its own scale) the HUD keeps the size
    // it has on the window: UI pixels are image pixels here.
    if touch.is_none() && w.height() > 0. {
        ui_scale.0 = size.y as f32 / w.height();
    }
    println!(
        "render_scale height={:?} render={}x{} screen={}x{} mode=upscaled-image hud=scaled-with-scene",
        scale.height, size.x, size.y, screen.x, screen.y
    );
}

/// Windowed runs only (offscreen captures keep their own image target).
pub fn install(app: &mut App, height: RenderHeight) {
    app.insert_resource(RenderScale {
        height,
        screen: UVec2::ZERO,
        render: UVec2::ZERO,
        image: None,
        display: None,
    })
    .add_systems(PostUpdate, apply);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_native_and_rows() {
        assert_eq!(RenderHeight::parse("native"), Ok(RenderHeight::Native));
        assert_eq!(RenderHeight::parse("NATIVE"), Ok(RenderHeight::Native));
        assert_eq!(RenderHeight::parse("1080"), Ok(RenderHeight::Rows(1080)));
        for bad in ["0", "100", "5000", "full", "-1080"] {
            assert!(RenderHeight::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn video_options_parse() {
        use bevy::window::PresentMode;
        assert_eq!(present_mode("FIFO"), Ok(PresentMode::Fifo));
        assert_eq!(present_mode("auto_no_vsync"), Ok(PresentMode::AutoNoVsync));
        assert!(present_mode("mailbox").is_err());
        assert_eq!(refresh_hz("system"), Ok(None));
        assert_eq!(refresh_hz("120"), Ok(Some(120.)));
        assert!(refresh_hz("10").is_err() && refresh_hz("fast").is_err());
        assert_eq!(frame_latency("1").map(|n| n.get()), Ok(1));
        assert!(frame_latency("0").is_err() && frame_latency("4").is_err());
    }

    #[test]
    fn caps_height_keeps_aspect_and_never_upscales() {
        let rows = RenderHeight::Rows(1080);
        // Xiaomi 13 Ultra WQHD+ landscape.
        assert_eq!(
            scaled_size(UVec2::new(3200, 1440), rows),
            Some(UVec2::new(2400, 1080))
        );
        // Already at or below the cap: straight to the window.
        assert_eq!(scaled_size(UVec2::new(2400, 1080), rows), None);
        assert_eq!(scaled_size(UVec2::new(1280, 800), rows), None);
        assert_eq!(
            scaled_size(UVec2::new(3200, 1440), RenderHeight::Native),
            None
        );
        assert_eq!(scaled_size(UVec2::ZERO, rows), None);
        // Odd ratios round to the nearest column.
        assert_eq!(
            scaled_size(UVec2::new(3088, 1440), RenderHeight::Rows(720)),
            Some(UVec2::new(1544, 720))
        );
        assert_eq!(
            scaled_size(UVec2::new(2001, 1000), RenderHeight::Rows(500)),
            Some(UVec2::new(1001, 500))
        );
    }
}
