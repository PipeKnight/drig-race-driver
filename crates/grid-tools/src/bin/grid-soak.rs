//! Headless soak/stress harness (docs/SOAK_TESTING.md). Runs seeded scenarios
//! through the app's driving path (`grid_tools::soak::Session`) with an
//! adversarial chaos player and reports world errors ("driving paused"),
//! panics, non-finite state, bodies leaving the world and implausible speeds.
//!
//! grid-soak quick [OPTIONS]            fixed seeds, a few minutes (pre-push)
//! grid-soak long [--hours H] [OPTIONS] many seeds, unattended, summary table
//! grid-soak run --seed N [OPTIONS]     one scenario in this process
//! grid-soak replay FILE.inputs.tsv     exact replay of recorded inputs
//! grid-soak replay-log FILE.log        replay a grid-app --drive playtest log
//!
//! Options: --out local/soak/DIR, --seeds A..B, --jobs J, --timeout SECONDS,
//! --max-cars N, --cars N, --route ROUTE, --ticks N, --grid-position P,
//! --assists SPEC, --chaos MODE, --props on|off, --car STEM, --opponent-cars
//! STEM[,STEM...] (the app's car options; long runs also mix BMW and Lacetti
//! fields by themselves), --keep (keep passing inputs), --game-root PATH.
//! Private inputs: GRID_CONTACT_DIRECTIONS, GRID_BODY_EVENT_PARAMETERS,
//! GRID_DEFORMATION_PARAMETERS, GRID_AI_CHARACTERISTICS (else local/*.bin).
use grid_tools::soak::{
    AppLog, Chaos, Kind, Mode, Private, Profile, Scenario, Session, Step, group_key,
};
use std::{
    collections::{BTreeMap, VecDeque},
    io::{BufRead, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Samples kept for a failure report.
const HISTORY: usize = 64;
const PROGRESS_EVERY: u64 = 250;

static PANIC: Mutex<Option<String>> = Mutex::new(None);

#[derive(Clone)]
struct Options {
    root: PathBuf,
    out: Option<PathBuf>,
    seeds: Option<(u64, u64)>,
    jobs: usize,
    timeout: Option<u64>,
    hours: f64,
    max_cars: usize,
    keep: bool,
    quiet: bool,
    profile: Profile,
    seed: Option<u64>,
    cars: Option<usize>,
    route: Option<String>,
    ticks: Option<u64>,
    grid_position: Option<usize>,
    assists: Option<String>,
    chaos: Option<Mode>,
    props: Option<bool>,
    car: Option<String>,
    opponent_cars: Option<String>,
    extra: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: std::env::var_os("GRID_GAME_ROOT").map_or_else(
                || PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\Grid"),
                PathBuf::from,
            ),
            out: None,
            seeds: None,
            jobs: 2,
            timeout: None,
            hours: 8.,
            max_cars: grid_tools::soak::MAX_SHARED_CARS,
            keep: false,
            quiet: false,
            profile: Profile::Quick,
            seed: None,
            cars: None,
            route: None,
            ticks: None,
            grid_position: None,
            assists: None,
            chaos: None,
            props: None,
            car: None,
            opponent_cars: None,
            extra: 0,
        }
    }
}

fn private_path(variable: &str, local: &str) -> Option<PathBuf> {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .or_else(|| Some(Path::new("local").join(local)).filter(|p| p.exists()))
}

fn load_private() -> Result<Private> {
    let mut private = Private::default();
    if let Some(p) = private_path("GRID_CONTACT_DIRECTIONS", "body-contact-directions.bin") {
        private.table = Some(std::fs::read(p)?);
    }
    if let Some(p) = private_path("GRID_BODY_EVENT_PARAMETERS", "body-impact-parameters.bin") {
        private.events = Some(grid_tools::body_impact_parameters::read(&std::fs::read(
            p,
        )?)?);
    }
    if let Some(p) = private_path("GRID_DEFORMATION_PARAMETERS", "deformation-parameters.bin") {
        private.deformation = Some(grid_tools::deformation_parameters::read(&std::fs::read(
            p,
        )?)?);
    }
    if let Some(p) = private_path("GRID_AI_CHARACTERISTICS", "ai-driver-characteristics.tsv") {
        private.characteristics =
            grid_tools::ai_driver::read_characteristics(&std::fs::read_to_string(p)?)?;
    }
    if private.table.is_none() {
        return Err("the soak needs body contacts: set GRID_CONTACT_DIRECTIONS (private CD01)".into());
    }
    Ok(private)
}

/// Artifacts stay under an ignored `local/` directory.
fn output_dir(options: &Options, name: &str) -> Result<PathBuf> {
    let out = options.out.clone().unwrap_or_else(|| {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Path::new("local").join("soak").join(format!("{name}-{stamp}"))
    });
    if !out.components().any(|c| c.as_os_str() == "local") {
        return Err("soak artifacts must stay under a local/ directory".into());
    }
    std::fs::create_dir_all(&out)?;
    Ok(out)
}

fn scenario_for(options: &Options, seed: u64, private: &Private) -> Result<Scenario> {
    let mut s = Scenario::generate(seed, options.profile, private.available(), options.max_cars);
    if let Some(cars) = options.cars {
        s.cars = cars;
        s.characteristics = private.available().characteristics && cars > 1;
        if s.grid_position > cars {
            s.grid_position = cars;
        }
        if s.laps.is_none() && cars > 1 {
            s.laps = Some(3);
        }
    }
    if let Some(route) = &options.route {
        s.route.clone_from(route);
    }
    if let Some(ticks) = options.ticks {
        s.ticks = ticks;
    }
    if let Some(p) = options.grid_position {
        s.grid_position = p;
    }
    if let Some(spec) = &options.assists {
        s.assists = grid_tools::source_vehicle::assists::Assists::parse(spec)?;
    }
    if let Some(props) = options.props {
        s.props = props;
    }
    if options.chaos.is_some() {
        s.chaos = options.chaos;
    }
    if let Some(car) = &options.car {
        s.car = grid_tools::car::Car::new(car)?;
    }
    if let Some(list) = &options.opponent_cars {
        s.opponent_cars = grid_tools::car::Car::list(list)?;
    }
    Ok(s)
}

#[derive(Clone)]
struct Sample {
    step: usize,
    segment: u64,
    tick: u64,
    mode: &'static str,
    line: String,
    position: [f32; 3],
    speed: f32,
    gear: u32,
    bodies: usize,
    pieces: usize,
    breaks: usize,
    contacts: usize,
    /// Lowest prop piece centre height (NaN without pieces).
    lowest_piece: f32,
}

#[derive(Clone, Debug)]
struct Failure {
    kind: Kind,
    text: String,
    /// Session tick (all segments) and tick within the segment.
    tick: u64,
    segment_tick: u64,
}

struct Outcome {
    failures: Vec<Failure>,
    ticks: u64,
    resets: u64,
    /// Player auto-resets onto the line (F341).
    auto_resets: u64,
    breaks: usize,
    max_bodies: usize,
    seconds: f64,
    digest: u64,
    /// First mismatch against logged poses (replay-log only).
    divergence: Option<String>,
    verified_poses: usize,
    /// World bodies (Markdown rows) at the first failure.
    bodies: Vec<String>,
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    let message = payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic".into());
    PANIC
        .lock()
        .ok()
        .and_then(|mut p| p.take())
        .unwrap_or(message)
}

/// Every world body (cars, then prop pieces) as Markdown table rows.
fn body_dump(session: &Session) -> Vec<String> {
    let snapshot = session.snapshot();
    snapshot
        .bodies
        .iter()
        .enumerate()
        .map(|(i, body)| {
            let f = |o: usize| f32::from_bits(body[o / 4]);
            format!(
                "| {i} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} |",
                if i < snapshot.cars { "car" } else { "piece" },
                f(0x1d0 + 48),
                f(0x1d0 + 52),
                f(0x1d0 + 56),
                f(0x220),
                f(0x224),
                f(0x228)
            )
        })
        .collect()
}

fn digest(session: &Session) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for i in 0..session.car_count() {
        let owner = &session.car(i).state.car.owner;
        for &word in owner.vehicle.iter().chain(&owner.physical) {
            hash = (hash ^ u64::from(word)).wrapping_mul(0x100_0000_01b3);
        }
    }
    hash
}

/// Where the steps come from.
enum Source {
    Chaos,
    Recorded(Vec<Step>),
}

struct Run<'a> {
    root: &'a Path,
    private: &'a Private,
    scenario: Scenario,
    source: Source,
    /// Inputs file (streamed) and report destination.
    out: Option<PathBuf>,
    /// Continue after a world error with the next segment (app logs: R).
    continue_after_error: bool,
    /// Keep the inputs file of a passing run.
    keep_inputs: bool,
    progress: bool,
    /// Logged player poses per segment (replay-log verification).
    poses: Vec<BTreeMap<u64, [f32; 16]>>,
}

impl Run<'_> {
    fn stem(&self) -> String {
        format!("seed-{}", self.scenario.seed)
    }

    fn execute(&self) -> Outcome {
        let started = Instant::now();
        let mut outcome = Outcome {
            failures: Vec::new(),
            ticks: 0,
            resets: 0,
            auto_resets: 0,
            breaks: 0,
            max_bodies: 0,
            seconds: 0.,
            digest: 0,
            divergence: None,
            verified_poses: 0,
            bodies: Vec::new(),
        };
        let mut history: VecDeque<Sample> = VecDeque::with_capacity(HISTORY);
        let mut inputs = self.out.as_ref().and_then(|out| {
            let path = out.join(format!("{}.inputs.tsv", self.stem()));
            let mut file = std::io::BufWriter::new(std::fs::File::create(path).ok()?);
            writeln!(file, "# grid-soak inputs version=1").ok()?;
            writeln!(file, "# scenario {}", self.scenario.header()).ok()?;
            writeln!(
                file,
                "# T throttle brake steering handbrake shift [mode]; R = reset (app R) before the next tick"
            )
            .ok()?;
            Some(file)
        });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
            let mut session = match Session::new(self.root, &self.scenario, self.private) {
                Ok(s) => s,
                Err(e) => {
                    outcome.failures.push(Failure {
                        kind: Kind::Setup,
                        text: e.to_string(),
                        tick: 0,
                        segment_tick: 0,
                    });
                    return Ok(());
                }
            };
            let mut chaos = match self.source {
                Source::Chaos => Some(Chaos::new(self.scenario.seed, &session)?),
                Source::Recorded(_) => None,
            };
            let mut recorded = match &self.source {
                Source::Recorded(steps) => Some(steps.iter()),
                Source::Chaos => None,
            };
            let trace: Option<u64> = std::env::var("GRID_SOAK_TRACE")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|&n| n > 0);
            let trace_body: Option<(Vec<usize>, u64)> =
                std::env::var("GRID_SOAK_TRACE_BODY").ok().and_then(|v| {
                    let (bodies, from) = v.split_once(':').unwrap_or((&v, "0"));
                    let bodies: Option<Vec<usize>> =
                        bodies.split(',').map(|b| b.parse::<usize>().ok()).collect();
                    Some((bodies?, from.parse().ok()?))
                });
            let mut segment = 0u64;
            let mut skip_segment = false;
            let mut step_index = 0usize;
            loop {
                let (step, mode) = match (&mut chaos, &mut recorded) {
                    (Some(chaos), _) => {
                        if session.total_ticks >= self.scenario.ticks {
                            break;
                        }
                        let d = chaos.decide(&session);
                        (d.step, d.mode.name())
                    }
                    (None, Some(steps)) => match steps.next() {
                        Some(s) => (*s, "replay"),
                        None => break,
                    },
                    _ => unreachable!(),
                };
                step_index += 1;
                if let Some(file) = &mut inputs {
                    writeln!(file, "{}", step.line(mode))?;
                }
                match step {
                    Step::Reset => {
                        session.reset();
                        if let Some(chaos) = &mut chaos {
                            chaos.on_reset();
                        }
                        segment += 1;
                        skip_segment = false;
                        outcome.resets += 1;
                    }
                    Step::Tick(controls, shift) => {
                        if skip_segment {
                            continue;
                        }
                        let tick = session.total_ticks + 1;
                        let segment_tick = session.runtime.ticks + 1;
                        let failure = match session.step(controls, shift) {
                            Err(e) => Some((Kind::WorldError, e.to_string())),
                            Ok(()) => session.check(),
                        };
                        let snapshot = session.snapshot();
                        let props = session.props();
                        let p = session.runtime.pose();
                        if history.len() == HISTORY {
                            history.pop_front();
                        }
                        history.push_back(Sample {
                            step: step_index,
                            segment,
                            tick: segment_tick,
                            mode,
                            line: step.line(""),
                            position: [p[12], p[13], p[14]],
                            speed: {
                                let v = session.runtime.velocity();
                                (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
                            },
                            gear: session.runtime.gear(),
                            bodies: snapshot.bodies.len(),
                            pieces: props.map_or(0, |p| p.pieces.len()),
                            breaks: props.map_or(0, |p| p.events.len()),
                            contacts: session.runtime.last_body_contacts,
                            lowest_piece: snapshot.bodies[snapshot.cars.min(snapshot.bodies.len())..]
                                .iter()
                                .map(|b| f32::from_bits(b[(0x1d0 + 52) / 4]))
                                .fold(f32::NAN, f32::min),
                        });
                        outcome.max_bodies = outcome.max_bodies.max(snapshot.bodies.len());
                        outcome.breaks = outcome.breaks.max(props.map_or(0, |p| p.events.len()));
                        if failure.is_none()
                            && let Some(logged) = self
                                .poses
                                .get(segment as usize)
                                .and_then(|m| m.get(&session.runtime.ticks))
                        {
                            let replayed = session.runtime.pose();
                            if replayed != *logged && outcome.divergence.is_none() {
                                let worst = (0..16)
                                    .map(|k| (replayed[k] - logged[k]).abs())
                                    .fold(0f32, f32::max);
                                outcome.divergence = Some(format!(
                                    "segment {segment} tick {}: pose differs by up to {worst}",
                                    session.runtime.ticks
                                ));
                            }
                            outcome.verified_poses += 1;
                        }
                        if let Some((kind, text)) = failure {
                            if outcome.bodies.is_empty() {
                                outcome.bodies = body_dump(&session);
                            }
                            outcome.failures.push(Failure {
                                kind,
                                text,
                                tick,
                                segment_tick,
                            });
                            if self.continue_after_error && kind == Kind::WorldError {
                                skip_segment = true;
                                continue;
                            }
                            break;
                        }
                        // GRID_SOAK_TRACE=N: one player sample every N ticks.
                        if let Some(every) = trace
                            && session.total_ticks.is_multiple_of(every)
                            && let Some(s) = history.back()
                        {
                            println!(
                                "soak_trace tick={} segment_tick={} mode={} input={} pos=({:.1},{:.1},{:.1}) speed={:.1} gear={} bodies={} breaks={} contacts={} lowest_piece_y={:.2}",
                                session.total_ticks,
                                s.tick,
                                s.mode,
                                s.line.trim_end().replace('\t', " "),
                                s.position[0],
                                s.position[1],
                                s.position[2],
                                s.speed,
                                s.gear,
                                s.bodies,
                                s.breaks,
                                s.contacts,
                                s.lowest_piece
                            );
                        }
                        // GRID_SOAK_TRACE_BODY=I,J..[:FROM]: these world bodies every tick from session tick FROM.
                        for (body, b) in trace_body
                            .iter()
                            .filter(|(_, from)| session.total_ticks >= *from)
                            .flat_map(|(bodies, _)| bodies)
                            .filter_map(|&i| Some((i, snapshot.bodies.get(i)?)))
                        {
                            let w = |o: usize| f32::from_bits(b[o / 4]);
                            let car = if body < snapshot.cars {
                                let wheels = session.car(body).wheels().map(|w| w.displacement);
                                let contacts = session
                                    .race
                                    .as_ref()
                                    .map_or(session.runtime.last_body_contacts, |r| {
                                        r.multi.body_contacts_of(body)
                                    });
                                format!(" contacts={contacts} wheels={wheels:.3?}")
                            } else {
                                String::new()
                            };
                            println!(
                                "soak_body tick={} body={body} pos=({:.3},{:.3},{:.3}) v=({:.3},{:.3},{:.3}) w=({:.3},{:.3},{:.3}) up=({:.3},{:.3},{:.3}){car}",
                                session.total_ticks,
                                w(0x1d0 + 48),
                                w(0x1d0 + 52),
                                w(0x1d0 + 56),
                                w(0x220),
                                w(0x224),
                                w(0x228),
                                w(0x210),
                                w(0x214),
                                w(0x218),
                                w(0x1d0 + 16),
                                w(0x1d0 + 20),
                                w(0x1d0 + 24),
                            );
                        }
                        if self.progress && session.total_ticks.is_multiple_of(PROGRESS_EVERY) {
                            println!("soak_progress tick={}", session.total_ticks);
                        }
                    }
                }
            }
            outcome.ticks = session.total_ticks;
            outcome.auto_resets = session.auto_resets;
            outcome.digest = digest(&session);
            Ok(())
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => outcome.failures.push(Failure {
                kind: Kind::Setup,
                text: e.to_string(),
                tick: 0,
                segment_tick: 0,
            }),
            Err(payload) => {
                let tick = history.back().map_or(0, |s| s.tick);
                outcome.failures.push(Failure {
                    kind: Kind::Panic,
                    text: panic_text(payload.as_ref()),
                    tick: outcome.ticks.max(tick),
                    segment_tick: tick,
                })
            }
        }
        outcome.seconds = started.elapsed().as_secs_f64();
        if let Some(mut file) = inputs {
            let _ = file.flush();
        }
        if let Some(out) = &self.out {
            let inputs = out.join(format!("{}.inputs.tsv", self.stem()));
            if outcome.failures.is_empty() && !self.keep_inputs {
                let _ = std::fs::remove_file(&inputs);
            } else {
                let _ = self.report(out, &outcome, &history, &inputs);
            }
        }
        outcome
    }

    fn report(
        &self,
        out: &Path,
        outcome: &Outcome,
        history: &VecDeque<Sample>,
        inputs: &Path,
    ) -> Result<()> {
        let path = out.join(format!("{}.failure.md", self.stem()));
        let mut f = std::io::BufWriter::new(std::fs::File::create(&path)?);
        writeln!(f, "# grid-soak failure, seed {}\n", self.scenario.seed)?;
        writeln!(f, "Scenario: `{}`\n", self.scenario.header())?;
        for failure in &outcome.failures {
            writeln!(
                f,
                "- **{}** at session tick {} (segment tick {}): {}",
                failure.kind.name(),
                failure.tick,
                failure.segment_tick,
                failure.text
            )?;
        }
        writeln!(
            f,
            "\nTicks run {}, resets {}, prop breaks {}, max world bodies {}, {:.1} s.",
            outcome.ticks, outcome.resets, outcome.breaks, outcome.max_bodies, outcome.seconds
        )?;
        writeln!(
            f,
            "\nReplay exactly: `grid-soak replay {}`\n",
            inputs.display()
        )?;
        if !outcome.bodies.is_empty() {
            writeln!(f, "## World bodies at the first failure\n")?;
            writeln!(f, "| body | what | x | y | z | vx | vy | vz |")?;
            writeln!(f, "|---|---|---|---|---|---|---|---|")?;
            for line in &outcome.bodies {
                writeln!(f, "{line}")?;
            }
            writeln!(f)?;
        }
        writeln!(f, "## Last {} samples (player car)\n", history.len())?;
        writeln!(
            f,
            "| step | segment | tick | mode | input (T throttle brake steering handbrake shift) | x | y | z | speed | gear | bodies | pieces | breaks | contacts |"
        )?;
        writeln!(f, "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")?;
        for s in history {
            writeln!(
                f,
                "| {} | {} | {} | {} | `{}` | {:.2} | {:.2} | {:.2} | {:.2} | {} | {} | {} | {} | {} |",
                s.step,
                s.segment,
                s.tick,
                s.mode,
                s.line.trim_end().replace('\t', " "),
                s.position[0],
                s.position[1],
                s.position[2],
                s.speed,
                s.gear,
                s.bodies,
                s.pieces,
                s.breaks,
                s.contacts
            )?;
        }
        Ok(())
    }
}

fn result_line(scenario: &Scenario, outcome: &Outcome) -> String {
    let failure = outcome.failures.first();
    format!(
        "soak_result seed={} status={} kind={} tick={} ticks={} resets={} auto_resets={} breaks={} max_bodies={} seconds={:.1} digest={:016x} cars={} route={} error={}",
        scenario.seed,
        if failure.is_some() { "fail" } else { "pass" },
        failure.map_or("-", |f| f.kind.name()),
        failure.map_or(0, |f| f.tick),
        outcome.ticks,
        outcome.resets,
        outcome.auto_resets,
        outcome.breaks,
        outcome.max_bodies,
        outcome.seconds,
        outcome.digest,
        scenario.cars,
        scenario.route,
        failure.map_or_else(|| "-".into(), |f| f.text.replace(['\n', '\r'], " "))
    )
}

/// Run on a 64 MiB stack like the other world tools.
fn on_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(f)
        .expect("soak worker")
        .join()
        .expect("soak worker panicked outside catch_unwind")
}

fn command_run(options: Options) -> Result<i32> {
    let seed = options.seed.ok_or("run needs --seed N")?;
    let private = load_private()?;
    let scenario = scenario_for(&options, seed, &private)?;
    let out = output_dir(&options, "run")?;
    if !options.quiet {
        println!("soak_scenario {}", scenario.header());
    }
    let root = options.root.clone();
    let keep = options.keep;
    let (line, failed) = on_big_stack(move || {
        let run = Run {
            root: &root,
            private: &private,
            scenario,
            source: Source::Chaos,
            out: Some(out),
            continue_after_error: false,
            keep_inputs: keep,
            progress: true,
            poses: Vec::new(),
        };
        let outcome = run.execute();
        (
            result_line(&run.scenario, &outcome),
            !outcome.failures.is_empty(),
        )
    });
    println!("{line}");
    Ok(i32::from(failed))
}

fn command_replay(options: Options, file: &Path) -> Result<i32> {
    let text = std::fs::read_to_string(file)?;
    let mut scenario = None;
    let mut steps = Vec::new();
    for line in text.lines() {
        if let Some(header) = line.strip_prefix("# scenario ") {
            scenario = Some(Scenario::parse(header)?);
        } else if !line.starts_with('#') && !line.trim().is_empty() {
            steps.push(Step::parse(line)?);
        }
    }
    let scenario = scenario.ok_or("inputs file lacks the # scenario header")?;
    println!("soak_replay {} steps={}", scenario.header(), steps.len());
    let private = load_private()?;
    let root = options.root.clone();
    // Reports only when asked for (--out), so a replay never overwrites the original.
    let out = match &options.out {
        Some(_) => Some(output_dir(&options, "replay")?),
        None => None,
    };
    let (line, failed) = on_big_stack(move || {
        let run = Run {
            root: &root,
            private: &private,
            scenario,
            source: Source::Recorded(steps),
            out,
            continue_after_error: false,
            keep_inputs: false,
            progress: false,
            poses: Vec::new(),
        };
        let outcome = run.execute();
        (
            result_line(&run.scenario, &outcome),
            !outcome.failures.is_empty(),
        )
    });
    println!("{line}");
    Ok(i32::from(failed))
}

fn command_replay_log(options: Options, file: &Path) -> Result<i32> {
    let log = AppLog::parse(&String::from_utf8_lossy(&std::fs::read(file)?))?;
    let mut scenario = log.scenario(options.grid_position, options.route.as_deref());
    scenario.seed = 0;
    let mut steps = log.steps();
    if options.extra > 0
        && let Some(Step::Tick(c, _)) = steps.iter().rev().find(|s| matches!(s, Step::Tick(..)))
    {
        let c = *c;
        steps.extend((0..options.extra).map(|_| Step::Tick(c, None)));
    }
    println!(
        "soak_replay_log {} segments={} steps={} explicit_inputs={} logged_failures={:?}",
        scenario.header(),
        log.segments.len(),
        steps.len(),
        log.explicit_inputs,
        log.segments
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.failure.as_ref().map(|f| (i, f.clone())))
            .collect::<Vec<_>>()
    );
    let poses = log
        .segments
        .iter()
        .map(|s| s.poses.iter().copied().collect())
        .collect();
    let private = load_private()?;
    let root = options.root.clone();
    let out = Some(output_dir(&options, "replay-log")?);
    let (lines, failed) = on_big_stack(move || {
        let run = Run {
            root: &root,
            private: &private,
            scenario,
            source: Source::Recorded(steps),
            out,
            continue_after_error: true,
            keep_inputs: true,
            progress: false,
            poses,
        };
        let outcome = run.execute();
        let mut lines = vec![result_line(&run.scenario, &outcome)];
        for f in &outcome.failures {
            lines.push(format!(
                "soak_replay_failure kind={} tick={} segment_tick={} error={}",
                f.kind.name(),
                f.tick,
                f.segment_tick,
                f.text
            ));
        }
        lines.push(format!(
            "soak_replay_verification poses_checked={} divergence={}",
            outcome.verified_poses,
            outcome.divergence.as_deref().unwrap_or("none")
        ));
        (lines, !outcome.failures.is_empty())
    });
    for line in lines {
        println!("{line}");
    }
    Ok(i32::from(failed))
}

#[derive(Clone)]
struct Row {
    scenario: Scenario,
    status: String,
    kind: String,
    tick: u64,
    ticks: u64,
    resets: u64,
    breaks: u64,
    max_bodies: u64,
    seconds: f64,
    error: String,
    digest: String,
    auto_resets: u64,
}

fn parse_result(scenario: &Scenario, line: &str) -> Row {
    let field = |key: &str| {
        line.split_whitespace()
            .find_map(|item| item.strip_prefix(key)?.strip_prefix('='))
            .unwrap_or("")
    };
    let num = |key: &str| field(key).parse().unwrap_or(0);
    Row {
        scenario: scenario.clone(),
        status: field("status").into(),
        kind: field("kind").into(),
        tick: num("tick"),
        ticks: num("ticks"),
        resets: num("resets"),
        breaks: num("breaks"),
        max_bodies: num("max_bodies"),
        seconds: field("seconds").parse().unwrap_or(0.),
        error: line
            .split_once(" error=")
            .map_or_else(String::new, |(_, e)| e.to_string()),
        digest: field("digest").into(),
        auto_resets: num("auto_resets"),
    }
}

fn write_summary(out: &Path, name: &str, rows: &[Row], started: Instant) -> Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(out.join("summary.md"))?);
    let failed = rows.iter().filter(|r| r.status != "pass").count();
    let ticks: u64 = rows.iter().map(|r| r.ticks).sum();
    writeln!(f, "# grid-soak {name} summary\n")?;
    writeln!(
        f,
        "{} scenarios, {failed} failed, {ticks} simulated ticks ({:.1} min simulated), {:.1} min wall.\n",
        rows.len(),
        ticks as f64 * 0.016 / 60.,
        started.elapsed().as_secs_f64() / 60.
    )?;
    let auto_resets: u64 = rows.iter().map(|r| r.auto_resets).sum();
    writeln!(f, "Player auto-resets onto the line (F341): {auto_resets}.\n")?;
    let mut groups: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.status != "pass") {
        let kind = Kind::parse(&r.kind).unwrap_or(Kind::Crash);
        groups.entry(group_key(kind, &r.error)).or_default().push(r);
    }
    writeln!(f, "## Distinct failures\n")?;
    if groups.is_empty() {
        writeln!(f, "None.\n")?;
    }
    for (key, rows) in &groups {
        writeln!(f, "- `{key}`: {} scenario(s)", rows.len())?;
        for r in rows.iter().take(8) {
            writeln!(
                f,
                "  - seed {} ({} cars, {}, tick {}): `grid-soak replay {}`",
                r.scenario.seed,
                r.scenario.cars,
                r.scenario.route.split('/').nth(2).unwrap_or(""),
                r.tick,
                out.join(format!("seed-{}.inputs.tsv", r.scenario.seed))
                    .display()
            )?;
        }
    }
    writeln!(f, "\n## Scenarios\n")?;
    writeln!(
        f,
        "| seed | route | cars | grid | assists | chaos | ticks | resets | breaks | max bodies | s | result | tick | error |"
    )?;
    writeln!(f, "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")?;
    for r in rows {
        let s = &r.scenario;
        writeln!(
            f,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {:.0} | {} | {} | {} |",
            s.seed,
            s.route.split('/').nth(2).unwrap_or(""),
            s.cars,
            s.grid_position,
            s.assists,
            s.chaos.map_or("mixed", Mode::name),
            r.ticks,
            r.resets,
            r.breaks,
            r.max_bodies,
            r.seconds,
            if r.status == "pass" {
                "pass".to_string()
            } else {
                r.kind.clone()
            },
            if r.status == "pass" { 0 } else { r.tick },
            if r.status == "pass" { "" } else { &r.error }
        )?;
    }
    let mut tsv = std::io::BufWriter::new(std::fs::File::create(out.join("summary.tsv"))?);
    writeln!(
        tsv,
        "seed\tstatus\tkind\ttick\tticks\tresets\tbreaks\tmax_bodies\tseconds\tdigest\terror\tscenario"
    )?;
    for r in rows {
        writeln!(
            tsv,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{}\t{}\t{}",
            r.scenario.seed,
            r.status,
            r.kind,
            r.tick,
            r.ticks,
            r.resets,
            r.breaks,
            r.max_bodies,
            r.seconds,
            r.digest,
            r.error,
            r.scenario.header()
        )?;
    }
    Ok(())
}

struct Child {
    scenario: Scenario,
    process: std::process::Child,
    lines: std::sync::mpsc::Receiver<String>,
    started: Instant,
    last_tick: u64,
    result: Option<String>,
    stderr: std::sync::Arc<Mutex<VecDeque<String>>>,
}

/// Runs scenarios in child processes (isolates aborts, stack overflows and
/// hangs), `jobs` at a time, rewriting the summary after each one.
fn orchestrate(options: Options, name: &str) -> Result<i32> {
    let private = load_private()?;
    let out = output_dir(&options, name)?;
    let exe = std::env::current_exe()?;
    let started = Instant::now();
    // Long runs stop at the deadline or after the last seed, whichever is first.
    let deadline = (options.profile == Profile::Long)
        .then(|| started + Duration::from_secs_f64(options.hours * 3600.));
    let (first, last) = options.seeds.unwrap_or(match options.profile {
        Profile::Quick => grid_tools::soak::QUICK_SEEDS,
        Profile::Long => (1001, u64::MAX),
    });
    println!(
        "soak_{name} out={} seeds={first}..{} jobs={} available={:?}",
        out.display(),
        if last == u64::MAX {
            "deadline".into()
        } else {
            last.to_string()
        },
        options.jobs,
        private.available()
    );
    let mut rows: Vec<Row> = Vec::new();
    let mut running: Vec<Child> = Vec::new();
    let mut next = first;
    loop {
        let open = next <= last && deadline.is_none_or(|d| Instant::now() < d);
        while open && running.len() < options.jobs.max(1) && next <= last {
            let scenario = scenario_for(&options, next, &private)?;
            let mut command = std::process::Command::new(&exe);
            command
                .arg("run")
                .arg("--seed")
                .arg(next.to_string())
                .arg("--profile")
                .arg(options.profile.name())
                .arg("--out")
                .arg(&out)
                .arg("--quiet")
                .arg("--game-root")
                .arg(&options.root)
                .arg("--max-cars")
                .arg(options.max_cars.to_string());
            for (flag, value) in [
                ("--cars", options.cars.map(|v| v.to_string())),
                ("--route", options.route.clone()),
                ("--ticks", options.ticks.map(|v| v.to_string())),
                ("--grid-position", options.grid_position.map(|v| v.to_string())),
                ("--assists", options.assists.clone()),
                ("--chaos", options.chaos.map(|m| m.name().to_string())),
                ("--props", options.props.map(|p| if p { "on" } else { "off" }.to_string())),
                ("--car", options.car.clone()),
                ("--opponent-cars", options.opponent_cars.clone()),
            ] {
                if let Some(value) = value {
                    command.arg(flag).arg(value);
                }
            }
            let mut process = command
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;
            let (tx, rx) = std::sync::mpsc::channel();
            let stdout = process.stdout.take().ok_or("child stdout")?;
            std::thread::spawn(move || {
                for line in std::io::BufReader::new(stdout).lines().map_while(|l| l.ok()) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
            let stderr_tail = std::sync::Arc::new(Mutex::new(VecDeque::new()));
            let tail = stderr_tail.clone();
            let stderr = process.stderr.take().ok_or("child stderr")?;
            std::thread::spawn(move || {
                for line in std::io::BufReader::new(stderr).lines().map_while(|l| l.ok()) {
                    if let Ok(mut t) = tail.lock() {
                        if t.len() == 20 {
                            t.pop_front();
                        }
                        t.push_back(line);
                    }
                }
            });
            if !options.quiet {
                println!("soak_start {}", scenario.header());
            }
            running.push(Child {
                scenario,
                process,
                lines: rx,
                started: Instant::now(),
                last_tick: 0,
                result: None,
                stderr: stderr_tail,
            });
            next += 1;
        }
        if running.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
        let mut finished = Vec::new();
        for (k, child) in running.iter_mut().enumerate() {
            while let Ok(line) = child.lines.try_recv() {
                if let Some(t) = line.strip_prefix("soak_progress tick=") {
                    child.last_tick = t.parse().unwrap_or(child.last_tick);
                } else if line.starts_with("soak_result ") {
                    child.result = Some(line);
                }
            }
            // Generous: a tick budget at 25 ms per car-tick plus set-up.
            let timeout = options.timeout.unwrap_or(
                120 + child.scenario.ticks * child.scenario.cars as u64 * 25 / 1000,
            );
            let status = child.process.try_wait()?;
            if status.is_some() || child.started.elapsed() > Duration::from_secs(timeout) {
                let hung = status.is_none();
                if hung {
                    let _ = child.process.kill();
                    let _ = child.process.wait();
                }
                // Drain what the reader thread still holds.
                std::thread::sleep(Duration::from_millis(100));
                while let Ok(line) = child.lines.try_recv() {
                    if line.starts_with("soak_result ") {
                        child.result = Some(line);
                    } else if let Some(t) = line.strip_prefix("soak_progress tick=") {
                        child.last_tick = t.parse().unwrap_or(child.last_tick);
                    }
                }
                let row = match &child.result {
                    Some(line) => parse_result(&child.scenario, line),
                    None => {
                        let stderr: Vec<String> = child
                            .stderr
                            .lock()
                            .map(|t| t.iter().cloned().collect())
                            .unwrap_or_default();
                        let (kind, error) = if hung {
                            (
                                Kind::Hang,
                                format!("no result after {timeout} s (last progress tick {})", child.last_tick),
                            )
                        } else {
                            (
                                Kind::Crash,
                                format!(
                                    "process ended with {:?} without a result: {}",
                                    status.and_then(|s| s.code()),
                                    stderr.last().cloned().unwrap_or_default()
                                ),
                            )
                        };
                        let report = out.join(format!("seed-{}.failure.md", child.scenario.seed));
                        let _ = std::fs::write(
                            &report,
                            format!(
                                "# grid-soak {} , seed {}\n\nScenario: `{}`\n\n{error}\n\nstderr tail:\n\n```\n{}\n```\n\nThe streamed inputs (if flushed) are in seed-{}.inputs.tsv; `grid-soak run --seed {} --profile {}` regenerates the run.\n",
                                kind.name(),
                                child.scenario.seed,
                                child.scenario.header(),
                                stderr.join("\n"),
                                child.scenario.seed,
                                child.scenario.seed,
                                child.scenario.profile.name()
                            ),
                        );
                        Row {
                            scenario: child.scenario.clone(),
                            status: "fail".into(),
                            kind: kind.name().into(),
                            tick: child.last_tick,
                            ticks: child.last_tick,
                            resets: 0,
                            breaks: 0,
                            max_bodies: 0,
                            seconds: child.started.elapsed().as_secs_f64(),
                            error,
                            digest: "-".into(),
                            auto_resets: 0,
                        }
                    }
                };
                println!(
                    "soak_done seed={} cars={} route={} {} {} tick={} ticks={} breaks={} max_bodies={} {:.0}s {}",
                    row.scenario.seed,
                    row.scenario.cars,
                    row.scenario.route.split('/').nth(2).unwrap_or(""),
                    row.status,
                    row.kind,
                    row.tick,
                    row.ticks,
                    row.breaks,
                    row.max_bodies,
                    row.seconds,
                    row.error
                );
                rows.push(row);
                finished.push(k);
            }
        }
        for k in finished.into_iter().rev() {
            running.remove(k);
        }
        if !rows.is_empty() {
            rows.sort_by_key(|r| r.scenario.seed);
            write_summary(&out, name, &rows, started)?;
        }
    }
    write_summary(&out, name, &rows, started)?;
    let failed = rows.iter().filter(|r| r.status != "pass").count();
    println!(
        "soak_summary scenarios={} failed={failed} summary={}",
        rows.len(),
        out.join("summary.md").display()
    );
    Ok(i32::from(failed > 0))
}

fn parse_options(args: &[String]) -> Result<(Options, Vec<String>)> {
    let mut o = Options::default();
    let mut rest = Vec::new();
    let mut i = 0;
    let value = |i: &mut usize| -> Result<String> {
        *i += 1;
        args.get(*i)
            .cloned()
            .ok_or_else(|| format!("{} needs a value", args[*i - 1]).into())
    };
    while i < args.len() {
        match args[i].as_str() {
            "--game-root" => o.root = value(&mut i)?.into(),
            "--out" => o.out = Some(value(&mut i)?.into()),
            "--seeds" => {
                let v = value(&mut i)?;
                let (a, b) = v.split_once("..").ok_or("--seeds needs A..B")?;
                o.seeds = Some((a.parse()?, b.parse()?));
            }
            "--seed" => o.seed = Some(value(&mut i)?.parse()?),
            "--jobs" => o.jobs = value(&mut i)?.parse()?,
            "--timeout" => o.timeout = Some(value(&mut i)?.parse()?),
            "--hours" => o.hours = value(&mut i)?.parse()?,
            "--max-cars" => o.max_cars = value(&mut i)?.parse()?,
            "--cars" => o.cars = Some(value(&mut i)?.parse()?),
            "--route" => o.route = Some(value(&mut i)?),
            "--car" => o.car = Some(value(&mut i)?),
            "--opponent-cars" => o.opponent_cars = Some(value(&mut i)?),
            "--ticks" => o.ticks = Some(value(&mut i)?.parse()?),
            "--grid-position" => o.grid_position = Some(value(&mut i)?.parse()?),
            "--assists" => o.assists = Some(value(&mut i)?),
            "--chaos" => o.chaos = Some(Mode::parse(&value(&mut i)?)?),
            "--profile" => o.profile = Profile::parse(&value(&mut i)?)?,
            "--props" => {
                o.props = Some(match value(&mut i)?.as_str() {
                    "on" => true,
                    "off" => false,
                    _ => return Err("--props needs on or off".into()),
                })
            }
            "--extra" => o.extra = value(&mut i)?.parse()?,
            "--keep" => o.keep = true,
            "--quiet" => o.quiet = true,
            other => rest.push(other.to_string()),
        }
        i += 1;
    }
    Ok((o, rest))
}

const USAGE: &str = "usage: grid-soak quick|long|run --seed N|replay FILE.inputs.tsv|replay-log FILE.log [options] (docs/SOAK_TESTING.md)";

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map_or_else(String::new, |l| format!(" at {}:{}", l.file(), l.line()));
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "non-string panic".into());
        let text = format!("{message}{location}");
        eprintln!("grid-soak panic: {text}");
        if let Ok(mut p) = PANIC.lock() {
            *p = Some(text);
        }
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = (|| -> Result<i32> {
        let (mut options, rest) = parse_options(&args)?;
        match rest.first().map(String::as_str) {
            Some("quick") => {
                options.profile = Profile::Quick;
                orchestrate(options, "quick")
            }
            Some("long") => {
                options.profile = Profile::Long;
                orchestrate(options, "long")
            }
            Some("run") => command_run(options),
            Some("replay") => {
                let file = rest.get(1).ok_or(USAGE)?;
                command_replay(options, Path::new(file))
            }
            Some("replay-log") => {
                let file = rest.get(1).ok_or(USAGE)?;
                command_replay_log(options, Path::new(file))
            }
            _ => Err(USAGE.into()),
        }
    })();
    match code {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("grid-soak: {error}");
            std::process::exit(2);
        }
    }
}
