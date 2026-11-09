//! The player's reset job (ResetLinesManager, docs/RESET_AND_RESPAWN_RESEARCH.md
//! 3 and 6): a request from the auto trigger (nearly still on the side or
//! roof for 2 s, 0x60D590) or from the pause menu's Reset Vehicle starts a
//! job (0x60D0E0). The job darkens the screen for [`reset_lines::FADE`] s,
//! ignores the player's input until it places the car at rest on the track
//! tracker's line halfway through the fade (0x612C90 event 4), keeps the
//! placed car out of car-to-car collisions for the rest of the fade and a
//! [`reset_lines::TAIL`] of 5 s, and ends once the car overlaps no other car
//! (event 9). AI cars are never reset (F332); a Demolition Derby car is
//! disqualified by the auto trigger instead. There is no time penalty.
//!
//! The line is the route's progress line ([`ResetLine`]), not the AI racing
//! line; the target is the car's tracker distance, moved ahead early in a
//! circuit race and clamped to a point-to-point finish (0x5EC0E0).
use super::{Controls, Runtime, multi::MultiRuntime};
use grid_formats::progress::{Progress, TrackKind};
use grid_world::{
    Error,
    reset_lines::{self, Detector, Job, JobEvent, Tracker, TrackerLine},
};
use std::path::Path;

/// The track tracker's line of a route and, on a point-to-point run, its
/// finish distance.
#[derive(Clone, Debug, PartialEq)]
pub struct ResetLine {
    pub line: TrackerLine,
    pub finish: Option<f32>,
}

impl ResetLine {
    /// The route's `progress_track.xml` points as [`crate::lap_progress`]
    /// prepares them: a circuit's from its finish around the lap, a
    /// point-to-point run's in travel order from its start, trimmed to the
    /// finish. `None` when the file has fewer than two points. [I] A drift
    /// freestyle arena's placeholder progress (2-3 gates, no valid lap
    /// gates) is read as the file holds it when its route ignores progress:
    /// the original's tracker has no other line there.
    pub fn load(
        root: &Path,
        route: &crate::route::Route,
    ) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let progress = match crate::lap_progress::load_route(root, route) {
            Ok(prepared) => prepared.progress,
            Err(_) if route.ignores_progress() => crate::lap_progress::read_route(root, route)?,
            Err(error) => return Err(error),
        };
        Ok(Self::from_progress(&progress))
    }

    /// From progress prepared by [`crate::lap_progress`]: a point-to-point
    /// run's single route says whether it goes against the file's order.
    pub fn from_progress(progress: &Progress) -> Option<Self> {
        let closed = progress.kind == TrackKind::Circuit;
        let reversed = !closed && progress.routes.first().is_some_and(|r| r.reverse);
        let points: Vec<_> = progress
            .points
            .iter()
            .map(|p| (p.position, p.distance))
            .collect();
        let line = TrackerLine::new(&points, closed, reversed, progress.total_distance)?;
        let finish = (!closed).then_some(progress.total_distance);
        Some(Self { line, finish })
    }
}

/// Reset state of the player's car. Shared by grid-app's drive tick and the
/// soak [`crate::soak::Session`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerReset {
    pub detector: Detector,
    /// Demolition Derby: the trigger disqualifies the car instead of
    /// resetting it (0x60D590 calls 0x9E64E0(6, 0) when +0x2E5C is 2) [V].
    pub demolition: bool,
    /// The car's trackpos on the reset line, updated every tick.
    pub tracker: Tracker,
    /// The running job, from the request until the car may collide again.
    pub job: Option<Job>,
    /// Placements so far: a view that follows the car cuts at each one (it
    /// happens in the dark of the fade) instead of swinging across.
    pub placements: u32,
    /// A pause-menu request waiting for the next update.
    requested: bool,
}

/// A performed placement: the line point and heading the car was placed at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reset {
    pub point: [f32; 3],
    pub heading: [f32; 3],
    /// Trackpos of the placement after the overlap retries.
    pub distance: f32,
    /// One-metre steps back along the line while the car overlapped another.
    pub retries: usize,
}

/// What an update did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A job started (the fade begins, input is ignored). `auto` for the
    /// still-and-tilted trigger, otherwise the pause menu.
    Started { auto: bool, target: f32 },
    /// The car was placed at the middle of the fade.
    Placed(Reset),
    /// The tail is over and the car overlaps no other car: collisions are
    /// back and the job is done.
    Ended,
    /// Derby: the auto trigger disqualified the car.
    Disqualified,
}

impl PlayerReset {
    /// No job yet; `demolition` for a Demolition Derby event.
    pub fn new(demolition: bool) -> Self {
        Self {
            demolition,
            ..Self::default()
        }
    }

    /// The pause menu's Reset Vehicle (`c_pause_reset_car`, the same
    /// ResetLinesManager request [I]); it starts at the next update.
    /// AUTHORED: refused while a job runs.
    pub fn request(&mut self) -> bool {
        let accepted = self.job.is_none();
        self.requested |= accepted;
        accepted
    }

    /// After a successful tick: track the player's car on `line`, start a
    /// job when the auto trigger fires or the menu asked, and run the job.
    /// `clock` is the race clock in seconds (the early-race target window).
    /// With a shared world, `player` is the runtime held outside slot 0 of
    /// `shared` (grid-app and the soak Session swap it in for the common
    /// tick). A retired player is never reset; without a line a request is
    /// dropped (there is nothing to place on).
    pub fn update(
        &mut self,
        player: &mut Runtime,
        mut shared: Option<&mut MultiRuntime>,
        line: Option<&ResetLine>,
        clock: f32,
    ) -> Result<Option<Event>, Error> {
        if shared.as_ref().is_some_and(|m| m.retirement(0).is_some()) {
            if self.job.take().is_some()
                && let Some(multi) = shared
            {
                multi.set_ghost(0, false);
            }
            self.requested = false;
            return Ok(None);
        }
        let pose = player.pose();
        let position = [pose[12], pose[13], pose[14]];
        if let Some(line) = line {
            self.tracker.update(&line.line, position);
        }
        if let Some(job) = &mut self.job {
            let target = job.target;
            return match job.update(super::DELTA) {
                None => Ok(None),
                Some(JobEvent::Place) => {
                    let Some(line) = line else {
                        self.job = None;
                        return Ok(None);
                    };
                    let reset = place(player, shared.as_deref_mut(), line, target)?;
                    if let Some(multi) = shared {
                        multi.set_ghost(0, true);
                    }
                    self.tracker.update(&line.line, reset.point);
                    self.placements = self.placements.wrapping_add(1);
                    Ok(Some(Event::Placed(reset)))
                }
                Some(JobEvent::TailOver) => {
                    if let Some(multi) = shared {
                        if multi.overlaps_other_car(0)? {
                            return Ok(None);
                        }
                        multi.set_ghost(0, false);
                    }
                    self.job = None;
                    Ok(Some(Event::Ended))
                }
            };
        }
        let [v, w] = [player.velocity(), player.angular_velocity()];
        let auto = self.detector.update(
            [v[0], v[1], v[2]],
            [w[0], w[1], w[2]],
            pose[5],
            super::DELTA,
        );
        let menu = std::mem::take(&mut self.requested);
        if !auto && !menu {
            return Ok(None);
        }
        if auto && self.demolition {
            player.disqualify();
            return Ok(Some(Event::Disqualified));
        }
        let Some(line) = line else {
            return Ok(None);
        };
        let target = reset_lines::target(
            &line.line,
            self.tracker.distance,
            position,
            clock,
            line.finish,
        );
        self.job = Some(Job::new(target));
        self.detector = Detector::default();
        Ok(Some(Event::Started { auto, target }))
    }

    /// Player input while a job runs: neutral until the placement (0x606990
    /// disables the car's controller at the fade start, event 2, and event 4
    /// enables it again).
    pub fn controls(&self, controls: Controls) -> Controls {
        if self.job.is_some_and(|j| j.controls_off()) {
            Controls::default()
        } else {
            controls
        }
    }

    /// Screen darkness of the reset fade, 0 (clear) to 1 (black).
    pub fn darkness(&self) -> f32 {
        self.job.map_or(0., |j| j.darkness())
    }

    /// `R` restarts the whole event (the derby mode stays).
    pub fn reset(&mut self) {
        *self = Self {
            demolition: self.demolition,
            ..Self::default()
        };
    }
}

/// The placement (0x612C90 event 4): the car at rest on the line at
/// `distance`, stepped back [`reset_lines::OVERLAP_STEP`] along the line
/// while it overlaps another car, at most [`reset_lines::OVERLAP_TRIES`]
/// tries (the last one stands: the car is a ghost until clear). A retired
/// player is refused.
pub fn place(
    player: &mut Runtime,
    shared: Option<&mut MultiRuntime>,
    line: &ResetLine,
    distance: f32,
) -> Result<Reset, Error> {
    let mut d = line.line.normalise(distance);
    let Some(multi) = shared else {
        let (point, heading) = line.line.point_at(d);
        player.place_at_rest(point, heading)?;
        return Ok(Reset {
            point,
            heading,
            distance: d,
            retries: 0,
        });
    };
    std::mem::swap(player, &mut multi.cars[0]);
    let mut retries = 0;
    let result = loop {
        let (point, heading) = line.line.point_at(d);
        if let Err(e) = multi.place_at_rest(0, point, heading) {
            break Err(e);
        }
        let clear = match multi.overlaps_other_car(0) {
            Ok(overlaps) => !overlaps,
            Err(e) => break Err(e),
        };
        if clear || retries + 1 >= reset_lines::OVERLAP_TRIES {
            break Ok(Reset {
                point,
                heading,
                distance: d,
                retries,
            });
        }
        retries += 1;
        d = line.line.step(d, -reset_lines::OVERLAP_STEP);
    };
    std::mem::swap(player, &mut multi.cars[0]);
    result
}

#[cfg(test)]
mod tests;
