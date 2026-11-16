//! Saved Type2 race effects for the two ordinary view slots.
use std::collections::BTreeMap;

use crate::{
    Result, fail,
    graphics_grade::{Balance, balance_from_parameters},
    postprocess::{self, Effects},
    xml_document,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaceEffect {
    pub balance: Balance,
    pub bloom_lower: f32,
    pub bloom_upper: f32,
    pub bloom_gain: f32,
    pub bloom_medium: f32,
    pub bloom_large: f32,
    pub bloom_history: f32,
    pub scene_amount: f32,
    pub bloom_amount: f32,
    pub softening_amount: f32,
    pub luminance_min: f32,
    pub luminance_max: f32,
    pub adapt_max: f32,
    pub luminance_threshold: f32,
    pub threshold_enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaceEffects {
    pub exterior: RaceEffect,
    pub interior: RaceEffect,
    pub pause: Option<PauseEffect>,
}

/// The pause target either uses a saved track effect or modifies the copied race view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PauseEffect {
    Saved(RaceEffect),
    Override(PauseOverride),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PauseOverride {
    pub luminance_min: f32,
    pub luminance_max: f32,
    pub adapt_max: f32,
    pub bloom_amount: f32,
    pub scene_amount: Option<f32>,
}

pub struct PauseSources<'a> {
    pub common: &'a [u8],
    pub render: &'a [u8],
    pub track_name: &'a str,
}

impl PauseEffect {
    pub fn blend(self, race: RaceEffect, share: f32) -> RaceEffect {
        let target = match self {
            Self::Saved(effect) => effect,
            Self::Override(effect) => RaceEffect {
                luminance_min: effect.luminance_min,
                luminance_max: effect.luminance_max,
                adapt_max: effect.adapt_max,
                bloom_amount: effect.bloom_amount,
                scene_amount: effect.scene_amount.unwrap_or(race.scene_amount),
                ..race
            },
        };
        if share == 0. {
            return race;
        }
        if share == 1. {
            return target;
        }
        let mix = |a, b| a * (1. - share) + b * share;
        RaceEffect {
            balance: race.balance.lerp(&target.balance, share),
            bloom_lower: mix(race.bloom_lower, target.bloom_lower),
            bloom_upper: mix(race.bloom_upper, target.bloom_upper),
            bloom_gain: mix(race.bloom_gain, target.bloom_gain),
            bloom_medium: mix(race.bloom_medium, target.bloom_medium),
            bloom_large: mix(race.bloom_large, target.bloom_large),
            bloom_history: mix(race.bloom_history, target.bloom_history),
            scene_amount: mix(race.scene_amount, target.scene_amount),
            bloom_amount: mix(race.bloom_amount, target.bloom_amount),
            softening_amount: mix(race.softening_amount, target.softening_amount),
            luminance_min: mix(race.luminance_min, target.luminance_min),
            luminance_max: mix(race.luminance_max, target.luminance_max),
            adapt_max: mix(race.adapt_max, target.adapt_max),
            luminance_threshold: mix(race.luminance_threshold, target.luminance_threshold),
            threshold_enabled: (if share > 0.5 { target } else { race }).threshold_enabled,
        }
    }
}

/// The original reader uses atof. Limit the supported conversion to
/// decimal control values and keep the accepted numeric prefix, e.g. the
/// shipped Spa interior brightness of "0.0.0173" becomes 0.0.
fn source_float(text: &str) -> Result<f32> {
    let text = text.trim();
    if text.len() <= 64 {
        for end in (1..=text.len()).rev() {
            if let Some(prefix) = text.get(..end)
                && let Ok(value) = prefix.parse::<f64>()
            {
                let value = value as f32;
                if value.is_finite() {
                    return Ok(value);
                }
                break;
            }
        }
    }
    fail(0, "nonfinite or missing source postprocess scalar")
}

type Parameters = BTreeMap<String, String>;

#[derive(Default)]
struct Slots {
    bloom: Parameters,
    mix: Parameters,
    tone: Parameters,
    balance: Parameters,
}

impl Slots {
    fn apply(&mut self, description: &Effects<'_>, id: i32, name: &str) -> Result<bool> {
        let mut selected = description.effects.iter().filter(|effect| effect.id == id);
        let Some(effect) = selected.next() else {
            return Ok(false);
        };
        if selected.next().is_some() || effect.name != name || effect.template.is_some() {
            return fail(effect.element, "invalid Type2 race effect identity");
        }
        if effect.chain.is_some_and(|chain| chain != "Type2") {
            return fail(effect.element, "unsupported race postprocess chain");
        }
        for (name, destination) in [
            ("BloomFromDsX4", &mut self.bloom),
            ("FinalMix", &mut self.mix),
            ("ToneMapFast", &mut self.tone),
            ("ColourBalance", &mut self.balance),
        ] {
            let mut groups = effect.groups.iter().filter(|group| group.name == name);
            let Some(group) = groups.next() else { continue };
            if groups.next().is_some() {
                return fail(group.element, "duplicate race postprocess group");
            }
            let mut parameters = BTreeMap::new();
            for parameter in &group.parameters {
                let Some(value) = parameter.payload else {
                    return fail(
                        parameter.element,
                        "missing race postprocess parameter value",
                    );
                };
                if parameters.insert(parameter.name, value).is_some() {
                    return fail(parameter.element, "duplicate race postprocess parameter");
                }
            }
            destination.extend(
                parameters
                    .into_iter()
                    .map(|(key, value)| (key.to_owned(), value.trim().to_owned())),
            );
        }
        Ok(true)
    }

    fn finish(&self) -> Result<RaceEffect> {
        fn scalar(map: &Parameters, key: &str, fallback: Option<f32>) -> Result<f32> {
            match map.get(key) {
                Some(value) => source_float(value),
                None => fallback.ok_or(crate::Error {
                    offset: 0,
                    message: "missing required Type2 parameter",
                }),
            }
        }

        let bloom = |key, default| scalar(&self.bloom, key, default);
        let mix = |key, default| scalar(&self.mix, key, default);
        let tone = |key| scalar(&self.tone, key, None);
        let threshold_enabled = match self.tone.get("toggleThreshold").map(String::as_str) {
            Some("true" | "1") => true,
            Some("false" | "0") | None => false,
            _ => return fail(0, "invalid source luminance threshold toggle"),
        };
        let effect = RaceEffect {
            balance: balance_from_parameters(&self.balance, source_float)?,
            bloom_lower: bloom("lowerThreshold", None)?,
            bloom_upper: bloom("upperThreshold", None)?,
            // Constructor defaults for omitted parameters in the global description (0x744690).
            bloom_gain: bloom("intensity", Some(1.))?,
            bloom_medium: bloom("mixMedium", None)?,
            bloom_large: bloom("mixLarge", None)?,
            bloom_history: bloom("lagFactor", Some(0.5))?,
            scene_amount: mix("colourAmount", None)?,
            bloom_amount: mix("bloomAmount", None)?,
            softening_amount: mix("softeningAmount", Some(0.))?,
            luminance_min: tone("lumMin")?,
            luminance_max: tone("lumMax")?,
            adapt_max: tone("adaptMax")?,
            luminance_threshold: scalar(&self.tone, "lumThreshold", Some(0.25))?,
            threshold_enabled,
        };
        if effect.bloom_upper <= effect.bloom_lower
            || !(0. ..=1.).contains(&effect.bloom_history)
            || !(0. ..=1.).contains(&effect.adapt_max)
            || effect.luminance_min < 0.
            || effect.luminance_max < effect.luminance_min
            || [
                effect.bloom_gain,
                effect.bloom_medium,
                effect.bloom_large,
                effect.scene_amount,
                effect.bloom_amount,
                effect.softening_amount,
            ]
            .into_iter()
            .any(|amount| !(0. ..=16.).contains(&amount))
        {
            return fail(0, "race postprocess parameter outside supported range");
        }
        Ok(effect)
    }
}

/// Apply the global description, followed by the selected route or track
/// description, field by field. The saved pause effect uses the same description layers.
pub fn read(
    global: &[u8],
    selected_track: Option<&[u8]>,
    pause_sources: Option<PauseSources<'_>>,
) -> Result<RaceEffects> {
    let global = xml_document::read(global)?;
    let global_effects = postprocess::read(&global.document())?;
    let mut exterior = Slots::default();
    let mut interior = Slots::default();
    if !exterior.apply(&global_effects, 2, "Effect_0000")?
        || !interior.apply(&global_effects, 3, "default_interior")?
    {
        return fail(0, "global Type2 race effect missing");
    }
    let mut saved_pause = Slots::default();
    let mut local_pause = false;
    if let Some(bytes) = selected_track {
        let owned = xml_document::read(bytes)?;
        let effects = postprocess::read(&owned.document())?;
        exterior.apply(&effects, 2, "Effect_0000")?;
        interior.apply(&effects, 3, "default_interior")?;
        if effects
            .effects
            .iter()
            .any(|effect| effect.name == "fepause_final")
        {
            saved_pause.apply(&global_effects, 17, "fepause_final")?;
            local_pause = saved_pause.apply(&effects, 17, "fepause_final")?;
        }
    }
    let pause = if local_pause {
        Some(PauseEffect::Saved(saved_pause.finish()?))
    } else {
        pause_sources
            .map(pause_override)
            .transpose()?
            .map(PauseEffect::Override)
    };
    Ok(RaceEffects {
        exterior: exterior.finish()?,
        interior: interior.finish()?,
        pause,
    })
}

/// Saved track metadata selects the dedicated ultra-dark pause profile.
fn uses_track_pause_tone_map(sources: &PauseSources<'_>) -> Result<bool> {
    let owned = xml_document::read(sources.common)?;
    let doc = owned.document();
    let root = &doc.elements[0];
    let Some(race_intro) = root
        .children
        .clone()
        .find(|&i| doc.elements[i].name == "race_intro")
    else {
        return Ok(false);
    };
    for track in doc.elements[race_intro].children.clone() {
        let node = &doc.elements[track];
        if node.name != "track" || postprocess::attr(track, node, "id")? != Some(sources.track_name)
        {
            continue;
        }
        for route in node.children.clone() {
            let node = &doc.elements[route];
            if node.name == "route" && postprocess::attr(route, node, "id")? == Some("pause") {
                return Ok(postprocess::attr(route, node, "use_track_tone_map")? == Some("true"));
            }
        }
    }
    Ok(false)
}

fn pause_override(sources: PauseSources<'_>) -> Result<PauseOverride> {
    let section = if uses_track_pause_tone_map(&sources)? {
        "pause_post_process_ultra_dark"
    } else {
        "pause_post_process"
    };
    let owned = xml_document::read(sources.render)?;
    let doc = owned.document();
    let root = &doc.elements[0];
    let unique_child = |parent: &crate::binxml::Element<'_>, name: &str| {
        let mut matches = parent
            .children
            .clone()
            .filter(|&i| doc.elements[i].name == name);
        match (matches.next(), matches.next()) {
            (Some(i), None) => Ok(i),
            _ => fail(0, "missing or ambiguous pause override"),
        }
    };
    let base = unique_child(root, section)?;
    let effect = unique_child(&doc.elements[base], "Effect")?;
    if postprocess::attr(effect, &doc.elements[effect], "name")? != Some("fepause_final") {
        return fail(effect, "unexpected pause source effect");
    }
    let group = |group: &str| {
        let mut selected = doc.elements[effect].children.clone().filter(|&i| {
            postprocess::attr(i, &doc.elements[i], "name")
                .ok()
                .flatten()
                == Some(group)
        });
        let g = selected.next().ok_or(crate::Error {
            offset: effect,
            message: "pause override group missing",
        })?;
        if selected.next().is_some() {
            return fail(g, "ambiguous pause override group");
        }
        postprocess::read_group(&doc, g)
    };
    let tone = group("ToneMapFast")?;
    let mix = group("FinalMix")?;
    let scene_amount = mix
        .parameters
        .iter()
        .any(|parameter| parameter.name == "colourAmount")
        .then(|| mix.float("colourAmount"))
        .transpose()?;
    let pause = PauseOverride {
        luminance_min: tone.float("lumMin")?,
        luminance_max: tone.float("lumMax")?,
        adapt_max: tone.float("adaptMax")?,
        bloom_amount: mix.float("bloomAmount")?,
        scene_amount,
    };
    // The saved ultra-dark case has reversed bounds. FastLumAdapt uses
    // min(max(luminance, min), max) in this order.
    if pause.luminance_min < 0.
        || pause.luminance_max < 0.
        || !(0. ..=1.).contains(&pause.adapt_max)
        || !(0. ..=16.).contains(&pause.bloom_amount)
        || scene_amount.is_some_and(|amount| !(0. ..=16.).contains(&amount))
    {
        return fail(effect, "pause override outside supported range");
    }
    Ok(pause)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and original postprocess data"]
    fn purchased_type2_race_postprocess_corpus() {
        let root = PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT"));
        let global = fs::read(root.join("postprocess/effects.xml")).unwrap();
        let render = fs::read(root.join("frontend/persistentdatarender.xml")).unwrap();
        let common = fs::read(root.join("frontend/persistentdatacommon.xml")).unwrap();

        let load = |route: &str| {
            let track = fs::read(root.join(route).join("effects.xml")).unwrap();
            let pause = PauseSources {
                common: &common,
                render: &render,
                track_name: route.split(['/', '\\']).nth(2).unwrap(),
            };
            read(&global, Some(&track), Some(pause)).unwrap_or_else(|err| panic!("{route}: {err}"))
        };
        let donington = load("tracks/europe/donington");
        let haruna = load("tracks/japan/haruna/route_1");
        let spa = load("tracks/europe/spa_francorchamps");
        let jarama = load("tracks/europe/jarama");
        assert_eq!(spa.interior.balance.brightness, 0.);
        assert_ne!(donington.exterior.bloom_lower, haruna.exterior.bloom_lower);
        assert_eq!(haruna.exterior.luminance_min, 0.4162);
        assert_ne!(jarama.exterior.bloom_lower, jarama.interior.bloom_lower);
        assert_eq!(
            donington.pause.unwrap(),
            PauseEffect::Override(PauseOverride {
                luminance_min: 0.2,
                luminance_max: 0.6,
                adapt_max: 0.5,
                bloom_amount: 0.4,
                scene_amount: None,
            })
        );
        let haruna_pause = haruna.pause.unwrap();
        assert!(matches!(haruna_pause, PauseEffect::Saved(_)));
        assert_eq!(haruna_pause.blend(haruna.exterior, 0.), haruna.exterior);
        let target = haruna_pause.blend(haruna.exterior, 1.);
        assert_eq!(target.luminance_min, 0.3526);
        assert_eq!(target.bloom_amount, 0.9364);
        assert!((haruna_pause.blend(haruna.exterior, 0.5).luminance_min - 0.3844).abs() < 1e-6);
        let shibuya = load("tracks/japan/shibuya");
        let ultra_dark = shibuya.pause.unwrap().blend(shibuya.exterior, 1.);
        assert_eq!(ultra_dark.luminance_min, 0.2);
        assert_eq!(ultra_dark.luminance_max, 0.0983);
        assert_eq!(ultra_dark.scene_amount, 1.6532);
        assert_eq!(ultra_dark.bloom_amount, 0.2659);
        assert_eq!(ultra_dark.balance, shibuya.exterior.balance);
        for track in fs::read_dir(root.join("tracks")).unwrap() {
            let region = track.unwrap().path();
            if !region.is_dir() || region.ends_with("frontend") {
                continue;
            }
            for track in fs::read_dir(region).unwrap() {
                let track = track.unwrap().path();
                if !track.is_dir() {
                    continue;
                }
                let base = track.join("effects.xml");
                if base.is_file() {
                    load(&track.strip_prefix(&root).unwrap().to_string_lossy());
                }
                for route in fs::read_dir(&track).unwrap() {
                    let route = route.unwrap().path();
                    if route.is_dir() && route.join("effects.xml").is_file() {
                        load(&route.strip_prefix(&root).unwrap().to_string_lossy());
                    }
                }
            }
        }
    }
}
