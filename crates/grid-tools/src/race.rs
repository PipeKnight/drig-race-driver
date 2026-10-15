//! Race standings for a shared-world event. AUTHORED classification (the
//! original race rules and position code are not recovered): every car's race
//! distance is measured along the AI racing line from the start-finish line,
//! unwrapped tick by tick; a car finishes when it has covered the lap count.
//! Positions order finished cars by finish time, then the rest by distance.
use crate::ai_driver::Line;

#[derive(Clone, Debug)]
struct Car {
    distance: f64,
    last_arc: f64,
    finish: Option<f64>,
    /// Finish time projected from distance left and speed when the event ended.
    estimated: bool,
}

#[derive(Clone, Debug)]
pub struct Standings {
    laps: u32,
    length: f64,
    start_arc: f64,
    cars: Vec<Car>,
}

/// `m:ss.mmm`.
pub fn format_time(seconds: f64) -> String {
    let milliseconds = (seconds.max(0.) * 1000.).round() as u64;
    format!(
        "{}:{:02}.{:03}",
        milliseconds / 60000,
        milliseconds / 1000 % 60,
        milliseconds % 1000
    )
}

fn wrap(delta: f64, length: f64) -> f64 {
    (delta + length / 2.).rem_euclid(length) - length / 2.
}

impl Standings {
    /// `start` is a point on the start-finish line; `positions` are the cars' grid positions.
    pub fn new(line: &Line, laps: u32, start: [f32; 3], positions: &[[f32; 3]]) -> Self {
        let length = f64::from(line.length);
        let start_arc = f64::from(line.arc(start));
        let mut standings = Self {
            laps,
            length,
            start_arc,
            cars: Vec::new(),
        };
        standings.reset(line, positions);
        standings
    }

    /// Restart from grid positions (cars behind the line start below zero).
    pub fn reset(&mut self, line: &Line, positions: &[[f32; 3]]) {
        self.cars = positions
            .iter()
            .map(|&p| {
                let arc = f64::from(line.arc(p));
                Car {
                    distance: wrap(arc - self.start_arc, self.length),
                    last_arc: arc,
                    finish: None,
                    estimated: false,
                }
            })
            .collect();
    }

    pub fn laps(&self) -> u32 {
        self.laps
    }
    pub fn len(&self) -> usize {
        self.cars.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cars.is_empty()
    }

    /// Advance with the cars' current positions at race time `time` (s from GO).
    /// Cars listed in `external` finish only through [`Self::finish`] (the
    /// player, whose finish follows the lap-timing gates).
    pub fn update(&mut self, line: &Line, positions: &[[f32; 3]], time: f64, external: &[usize]) {
        let target = f64::from(self.laps) * self.length;
        for (i, (car, &p)) in self.cars.iter_mut().zip(positions).enumerate() {
            if car.finish.is_some() {
                continue;
            }
            let arc = f64::from(line.arc(p));
            car.distance += wrap(arc - car.last_arc, self.length);
            car.last_arc = arc;
            if car.distance >= target && !external.contains(&i) {
                car.finish = Some(time);
            }
        }
    }

    pub fn finish(&mut self, car: usize, time: f64) {
        if let Some(c) = self.cars.get_mut(car)
            && c.finish.is_none()
        {
            c.finish = Some(time);
        }
    }

    /// End of the event: every unfinished car gets a finish time projected
    /// from its remaining distance at its current speed (m/s, floor 1).
    pub fn estimate_remaining(&mut self, speeds: &[f64], time: f64) {
        let target = f64::from(self.laps) * self.length;
        for (car, &speed) in self.cars.iter_mut().zip(speeds) {
            if car.finish.is_none() {
                car.finish = Some(time + (target - car.distance).max(0.) / speed.max(1.));
                car.estimated = true;
            }
        }
    }

    pub fn finished(&self, car: usize) -> Option<f64> {
        self.cars.get(car).and_then(|c| c.finish)
    }

    /// Completed race distance of `car` in laps (may be negative before the line).
    pub fn laps_covered(&self, car: usize) -> f64 {
        self.cars[car].distance / self.length
    }

    /// Car indices from first to last.
    pub fn order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.cars.len()).collect();
        order.sort_by(|&a, &b| {
            let (ca, cb) = (&self.cars[a], &self.cars[b]);
            match (ca.finish, cb.finish) {
                (Some(x), Some(y)) => x.total_cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => cb.distance.total_cmp(&ca.distance),
            }
        });
        order
    }

    /// Race distance (m) to the car directly ahead and directly behind `car`
    /// in the running order; `None` at the ends or once either has finished.
    pub fn neighbour_gaps(&self, car: usize) -> (Option<f64>, Option<f64>) {
        let order = self.order();
        let Some(at) = order.iter().position(|&c| c == car) else {
            return (None, None);
        };
        let gap = |other: usize| {
            let (a, b) = (&self.cars[car], &self.cars[other]);
            (a.finish.is_none() && b.finish.is_none()).then(|| (b.distance - a.distance).abs())
        };
        (
            at.checked_sub(1).and_then(|i| gap(order[i])),
            order.get(at + 1).and_then(|&c| gap(c)),
        )
    }

    /// One-based position of `car`.
    pub fn position(&self, car: usize) -> usize {
        self.order()
            .iter()
            .position(|&c| c == car)
            .map_or(0, |p| p + 1)
    }

    /// Classification lines; unfinished cars show the distance still to cover.
    pub fn classification(&self, names: &[String]) -> Vec<String> {
        let order = self.order();
        let leader = order.first().and_then(|&c| self.cars[c].finish);
        let target = f64::from(self.laps) * self.length;
        order
            .iter()
            .enumerate()
            .map(|(rank, &c)| {
                let car = &self.cars[c];
                let mark = if car.estimated { " (est.)" } else { "" };
                let result = match (car.finish, leader) {
                    (Some(t), Some(l)) if rank > 0 => format!("+{:.3}{mark}", t - l),
                    (Some(t), _) => format!("{}{mark}", format_time(t)),
                    (None, _) => {
                        let remaining = (target - car.distance).max(0.);
                        format!("running, {:.0} m to go", remaining)
                    }
                };
                format!(
                    "{}. {}  {}",
                    rank + 1,
                    names.get(c).map_or("?", String::as_str),
                    result
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn wrap_keeps_small_deltas_across_the_seam() {
        assert_eq!(super::wrap(3940. - 10., 3944.), 3930. - 3944.);
        assert_eq!(super::wrap(-3930., 3944.), 14.);
        assert_eq!(super::wrap(5., 3944.), 5.);
    }
}
