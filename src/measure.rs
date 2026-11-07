//! Automatic measurements, like the "Measure" menu of a bench oscilloscope.
//!
//! All values come from the samples in a time window (the screen, the whole
//! record or the span between the time cursors).
//!
//! - Top and base are the most common levels in the upper and lower half of
//!   the signal (histogram), so that overshoot and ringing do not change them.
//!   A sine has no flat top; then top and base are close to max and min.
//! - Edges are found with hysteresis: a rising edge goes from below the 10%
//!   level to above the 90% level (between base and top). The time of an edge
//!   is its crossing of the 50% level.

use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

/// One automatic measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Measurement {
    Frequency,
    Period,
    Vpp,
    Vmax,
    Vmin,
    Vtop,
    Vbase,
    Vamp,
    Mean,
    Rms,
    RiseTime,
    FallTime,
    PositiveWidth,
    NegativeWidth,
    DutyCycle,
}

/// Unit of a measurement, for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Volt,
    Second,
    Hertz,
    Percent,
}

impl Measurement {
    /// All measurements, in display order.
    pub const ALL: [Self; 15] = [
        Self::Frequency,
        Self::Period,
        Self::Vpp,
        Self::Vmax,
        Self::Vmin,
        Self::Vtop,
        Self::Vbase,
        Self::Vamp,
        Self::Mean,
        Self::Rms,
        Self::RiseTime,
        Self::FallTime,
        Self::PositiveWidth,
        Self::NegativeWidth,
        Self::DutyCycle,
    ];

    /// Short label for the readout.
    pub fn label(self) -> &'static str {
        match self {
            Self::Frequency => "Freq",
            Self::Period => "Period",
            Self::Vpp => "Vpp",
            Self::Vmax => "Vmax",
            Self::Vmin => "Vmin",
            Self::Vtop => "Vtop",
            Self::Vbase => "Vbase",
            Self::Vamp => "Vamp",
            Self::Mean => "Mean",
            Self::Rms => "RMS",
            Self::RiseTime => "Rise",
            Self::FallTime => "Fall",
            Self::PositiveWidth => "+Width",
            Self::NegativeWidth => "−Width",
            Self::DutyCycle => "+Duty",
        }
    }

    /// Description for the menu.
    pub fn description(self) -> &'static str {
        match self {
            Self::Frequency => "Frequency: 1 / period",
            Self::Period => "Period: mean time between rising edges",
            Self::Vpp => "Peak-to-peak voltage: Vmax − Vmin",
            Self::Vmax => "Highest sample",
            Self::Vmin => "Lowest sample",
            Self::Vtop => "Top level (most common high level)",
            Self::Vbase => "Base level (most common low level)",
            Self::Vamp => "Amplitude: Vtop − Vbase",
            Self::Mean => "Mean of the samples",
            Self::Rms => "RMS of the samples (AC + DC)",
            Self::RiseTime => "Rise time, 10% to 90%",
            Self::FallTime => "Fall time, 90% to 10%",
            Self::PositiveWidth => "Positive pulse width, at 50%",
            Self::NegativeWidth => "Negative pulse width, at 50%",
            Self::DutyCycle => "Positive duty cycle: +Width / period",
        }
    }

    pub fn unit(self) -> Unit {
        match self {
            Self::Frequency => Unit::Hertz,
            Self::Period
            | Self::RiseTime
            | Self::FallTime
            | Self::PositiveWidth
            | Self::NegativeWidth => Unit::Second,
            Self::DutyCycle => Unit::Percent,
            _ => Unit::Volt,
        }
    }
}

/// The samples that the measurements use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Window {
    /// The time span shown on the screen now.
    #[default]
    Screen,
    /// All the samples in the file.
    Record,
    /// Between the time cursors T1 and T2.
    Cursors,
}

impl Window {
    pub const ALL: [Self; 3] = [Self::Screen, Self::Record, Self::Cursors];

    pub fn label(self) -> &'static str {
        match self {
            Self::Screen => "Screen",
            Self::Record => "Record",
            Self::Cursors => "T1–T2",
        }
    }
}

/// What the user wants to see. Saved between sessions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Selected measurements (shown in `Measurement::ALL` order).
    pub selected: Vec<Measurement>,
    /// Labels of the channels not to measure ("CH2"); the others are measured.
    pub excluded: Vec<String>,
    pub window: Window,
    /// Show statistics over the cycles in the window (mean, min, max, σ, n).
    pub stats: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            selected: vec![Measurement::Frequency, Measurement::Vpp],
            excluded: Vec::new(),
            window: Window::Screen,
            stats: false,
        }
    }
}

impl Settings {
    /// Measure the channel with label `label`?
    pub fn is_measured(&self, label: &str) -> bool {
        !self.excluded.iter().any(|e| e == label)
    }

    pub fn set_measured(&mut self, label: &str, on: bool) {
        self.excluded.retain(|e| e != label);
        if !on {
            self.excluded.push(label.to_owned());
        }
    }

    pub fn is_selected(&self, m: Measurement) -> bool {
        self.selected.contains(&m)
    }

    pub fn set_selected(&mut self, m: Measurement, on: bool) {
        self.selected.retain(|x| *x != m);
        if on {
            self.selected.push(m);
        }
    }

    /// The selected measurements in display order.
    pub fn shown(&self) -> impl Iterator<Item = Measurement> + '_ {
        Measurement::ALL
            .into_iter()
            .filter(|m| self.is_selected(*m))
    }
}

/// One edge: the time it crosses 50%, and its 10%–90% duration.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Edge {
    rising: bool,
    t_mid: f64,
    duration: f64,
}

/// All the measurements of one channel in one window.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Results {
    pub vmax: Option<f64>,
    pub vmin: Option<f64>,
    pub vtop: Option<f64>,
    pub vbase: Option<f64>,
    pub mean: Option<f64>,
    pub rms: Option<f64>,
    pub period: Option<f64>,
    pub rise: Option<f64>,
    pub fall: Option<f64>,
    pub pos_width: Option<f64>,
    pub neg_width: Option<f64>,
    /// The value of each cycle (or edge) in the window, for statistics.
    pub cycles: Cycles,
}

/// Values of each cycle in the window: periods, widths, rise and fall times
/// and duty cycles (%), after the glitch filter.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cycles {
    pub periods: Vec<f64>,
    pub rises: Vec<f64>,
    pub falls: Vec<f64>,
    pub pos_widths: Vec<f64>,
    pub neg_widths: Vec<f64>,
    pub duties: Vec<f64>,
}

/// Statistics of a list of values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    /// Standard deviation.
    pub std_dev: f64,
    pub count: usize,
}

impl Stats {
    /// `None` for an empty list.
    pub fn of(values: &[f64]) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;
        let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
        Some(Self {
            mean,
            min: values.iter().copied().fold(f64::INFINITY, f64::min),
            max: values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            std_dev: var.sqrt(),
            count: values.len(),
        })
    }
}

impl Results {
    /// The value of measurement `m` in each cycle, for the time
    /// measurements. `None` for the voltage measurements (one value per window).
    pub fn per_cycle(&self, m: Measurement) -> Option<Vec<f64>> {
        let c = &self.cycles;
        Some(match m {
            Measurement::Period => c.periods.clone(),
            Measurement::Frequency => c.periods.iter().map(|p| 1.0 / p).collect(),
            Measurement::RiseTime => c.rises.clone(),
            Measurement::FallTime => c.falls.clone(),
            Measurement::PositiveWidth => c.pos_widths.clone(),
            Measurement::NegativeWidth => c.neg_widths.clone(),
            Measurement::DutyCycle => c.duties.clone(),
            _ => return None,
        })
    }

    /// Value of one measurement, `None` if it cannot be measured
    /// (for example the frequency of a signal with fewer than two edges).
    pub fn get(&self, m: Measurement) -> Option<f64> {
        let sub = |a: Option<f64>, b: Option<f64>| a.zip(b).map(|(a, b)| a - b);
        match m {
            Measurement::Frequency => self.period.map(|p| 1.0 / p),
            Measurement::Period => self.period,
            Measurement::Vpp => sub(self.vmax, self.vmin),
            Measurement::Vmax => self.vmax,
            Measurement::Vmin => self.vmin,
            Measurement::Vtop => self.vtop,
            Measurement::Vbase => self.vbase,
            Measurement::Vamp => sub(self.vtop, self.vbase),
            Measurement::Mean => self.mean,
            Measurement::Rms => self.rms,
            Measurement::RiseTime => self.rise,
            Measurement::FallTime => self.fall,
            Measurement::PositiveWidth => self.pos_width,
            Measurement::NegativeWidth => self.neg_width,
            Measurement::DutyCycle => self.pos_width.zip(self.period).map(|(w, p)| w / p * 100.0),
        }
    }
}

/// Number of histogram bins for top and base.
const BINS: usize = 100;

/// Measure the samples with a time in `window`.
///
/// `min_amplitude` (volts): if top − base is smaller, the signal is taken as
/// noise and no edges, period or widths are measured.
pub fn measure(points: &[[f64; 2]], window: &RangeInclusive<f64>, min_amplitude: f64) -> Results {
    let (start, end) = (*window.start(), *window.end());
    let a = points.partition_point(|p| p[0] < start);
    let b = points.partition_point(|p| p[0] <= end);
    let pts = &points[a..b.max(a)];
    if pts.is_empty() {
        return Results::default();
    }

    let n = pts.len() as f64;
    let (vmin, vmax) = pts
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p[1]), hi.max(p[1]))
        });
    let mean = pts.iter().map(|p| p[1]).sum::<f64>() / n;
    let rms = (pts.iter().map(|p| p[1] * p[1]).sum::<f64>() / n).sqrt();
    let (vbase, vtop) = top_and_base(pts, vmin, vmax);

    let mut r = Results {
        vmax: Some(vmax),
        vmin: Some(vmin),
        vtop: Some(vtop),
        vbase: Some(vbase),
        mean: Some(mean),
        rms: Some(rms),
        ..Results::default()
    };
    if vtop - vbase < min_amplitude || vtop <= vbase {
        return r;
    }

    let edges = remove_glitches(find_edges(pts, vbase, vtop));
    // Glitches (short spikes through the whole range) make extra short edges
    // and pulses, so use a mean that ignores values far from the median.
    let avg = |v: &[f64]| robust_mean(v, 0.5);
    let durations = |rising: bool| -> Vec<f64> {
        edges
            .iter()
            .filter(|e| e.rising == rising)
            .map(|e| e.duration)
            .collect()
    };
    r.rise = avg(&durations(true));
    r.fall = avg(&durations(false));

    // Period: mean time between edges of the same direction.
    let period_of = |rising: bool| {
        let t: Vec<f64> = edges
            .iter()
            .filter(|e| e.rising == rising)
            .map(|e| e.t_mid)
            .collect();
        let gaps: Vec<f64> = t.windows(2).map(|p| p[1] - p[0]).collect();
        robust_mean(&gaps, 0.2)
    };
    r.period = period_of(true).or_else(|| period_of(false));

    // Widths: from an edge to the next edge (which has the other direction).
    let width = |rising: bool| {
        let w: Vec<f64> = edges
            .windows(2)
            .filter(|p| p[0].rising == rising)
            .map(|p| p[1].t_mid - p[0].t_mid)
            .collect();
        avg(&w)
    };
    r.pos_width = width(true);
    r.neg_width = width(false);

    // Each cycle: from a rising edge to the next rising edge.
    let mut cycles = Cycles {
        rises: durations(true),
        falls: durations(false),
        ..Cycles::default()
    };
    for p in edges.windows(2) {
        let w = p[1].t_mid - p[0].t_mid;
        if p[0].rising {
            cycles.pos_widths.push(w);
        } else {
            cycles.neg_widths.push(w);
        }
    }
    for p in edges.windows(3).filter(|p| p[0].rising) {
        let period = p[2].t_mid - p[0].t_mid;
        if period > 0.0 {
            cycles.periods.push(period);
            cycles
                .duties
                .push((p[1].t_mid - p[0].t_mid) / period * 100.0);
        }
    }
    r.cycles = cycles;
    r
}

/// What [`find_events`] looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventKind {
    Rising,
    Falling,
    /// A pulse much shorter than the others (two edges closer than 10% of
    /// the typical time between edges).
    Glitch,
}

impl EventKind {
    pub const ALL: [Self; 3] = [Self::Rising, Self::Falling, Self::Glitch];

    pub fn label(self) -> &'static str {
        match self {
            Self::Rising => "Rising edges",
            Self::Falling => "Falling edges",
            Self::Glitch => "Glitches",
        }
    }
}

/// Times of the events of `kind` in the whole record, in time order. Edges
/// use the 50% level and the same hysteresis as the measurements. A signal
/// with an amplitude below `min_amplitude` has no events.
pub fn find_events(points: &[[f64; 2]], kind: EventKind, min_amplitude: f64) -> Vec<f64> {
    if points.len() < 2 {
        return Vec::new();
    }
    let (vmin, vmax) = points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p[1]), hi.max(p[1]))
        });
    let (vbase, vtop) = top_and_base(points, vmin, vmax);
    if vtop - vbase < min_amplitude || vtop <= vbase {
        return Vec::new();
    }
    let (edges, glitches) = split_glitches(find_edges(points, vbase, vtop));
    match kind {
        EventKind::Rising => edges.iter().filter(|e| e.rising).map(|e| e.t_mid).collect(),
        EventKind::Falling => edges
            .iter()
            .filter(|e| !e.rising)
            .map(|e| e.t_mid)
            .collect(),
        EventKind::Glitch => glitches,
    }
}

/// Remove glitches: two edges closer than 10% of the median time between
/// edges are a short spike, not a real pulse.
fn remove_glitches(edges: Vec<Edge>) -> Vec<Edge> {
    split_glitches(edges).0
}

/// The edges without the glitches, and the time of each glitch (its first edge).
fn split_glitches(mut edges: Vec<Edge>) -> (Vec<Edge>, Vec<f64>) {
    let mut glitches = Vec::new();
    let mut gaps: Vec<f64> = edges.windows(2).map(|p| p[1].t_mid - p[0].t_mid).collect();
    gaps.sort_by(f64::total_cmp);
    let Some(&median) = gaps.get(gaps.len() / 2) else {
        return (edges, glitches);
    };
    let mut i = 0;
    while i + 1 < edges.len() {
        if edges[i + 1].t_mid - edges[i].t_mid < 0.1 * median {
            glitches.push(edges[i].t_mid);
            edges.drain(i..i + 2);
        } else {
            i += 1;
        }
    }
    (edges, glitches)
}

/// Mean of the values within `tolerance` (a fraction) of the median.
/// `None` if `values` is empty.
fn robust_mean(values: &[f64], tolerance: f64) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let median = *sorted.get(sorted.len() / 2)?;
    let band = median.abs() * tolerance;
    let kept: Vec<f64> = sorted
        .into_iter()
        .filter(|v| (v - median).abs() <= band)
        .collect();
    Some(kept.iter().sum::<f64>() / kept.len() as f64)
}

/// Base and top: the most common levels in the lower and upper half.
fn top_and_base(pts: &[[f64; 2]], vmin: f64, vmax: f64) -> (f64, f64) {
    let span = vmax - vmin;
    if span <= 0.0 {
        return (vmin, vmax);
    }
    let mut hist = [0usize; BINS];
    for p in pts {
        let i = (((p[1] - vmin) / span) * BINS as f64) as usize;
        hist[i.min(BINS - 1)] += 1;
    }
    let mode = |range: std::ops::Range<usize>| {
        range
            .max_by_key(|&i| hist[i])
            .map(|i| vmin + (i as f64 + 0.5) / BINS as f64 * span)
    };
    let base = mode(0..BINS / 2).unwrap_or(vmin).max(vmin);
    // `max_by_key` returns the last maximum, so search the top half from the top
    // down to prefer the higher level on a tie.
    let top = (BINS / 2..BINS)
        .rev()
        .max_by_key(|&i| hist[i])
        .map(|i| vmin + (i as f64 + 0.5) / BINS as f64 * span)
        .unwrap_or(vmax)
        .min(vmax);
    (base, top)
}

/// Time where the line from `a` to `b` crosses `level`.
fn crossing(a: [f64; 2], b: [f64; 2], level: f64) -> f64 {
    if b[1] == a[1] {
        return b[0];
    }
    a[0] + (b[0] - a[0]) * (level - a[1]) / (b[1] - a[1])
}

/// Edges with hysteresis between the 10% and 90% levels.
fn find_edges(pts: &[[f64; 2]], base: f64, top: f64) -> Vec<Edge> {
    let amp = top - base;
    let (low, mid, high) = (base + 0.1 * amp, base + 0.5 * amp, base + 0.9 * amp);
    #[derive(PartialEq)]
    enum State {
        Unknown,
        Low,
        High,
    }
    let mut state = State::Unknown;
    // Last sample on the far side of the start level of the next edge.
    let mut last_out = 0usize;
    let mut edges = Vec::new();
    for (i, p) in pts.iter().enumerate() {
        let v = p[1];
        match state {
            State::Unknown | State::Low if v < low => {
                state = State::Low;
                last_out = i;
            }
            State::Unknown | State::High if v > high => {
                state = State::High;
                last_out = i;
            }
            State::Low if v > high => {
                // Rising edge from sample `last_out` (below 10%) to `i` (above 90%).
                let j = last_out;
                let t10 = crossing(pts[j], pts[j + 1], low);
                let t90 = crossing(pts[i - 1], pts[i], high);
                let k = (j..i).find(|&k| pts[k + 1][1] >= mid).unwrap_or(i - 1);
                let t50 = crossing(pts[k], pts[k + 1], mid);
                edges.push(Edge {
                    rising: true,
                    t_mid: t50,
                    duration: t90 - t10,
                });
                state = State::High;
                last_out = i;
            }
            State::High if v < low => {
                let j = last_out;
                let t90 = crossing(pts[j], pts[j + 1], high);
                let t10 = crossing(pts[i - 1], pts[i], low);
                let k = (j..i).find(|&k| pts[k + 1][1] <= mid).unwrap_or(i - 1);
                let t50 = crossing(pts[k], pts[k + 1], mid);
                edges.push(Edge {
                    rising: false,
                    t_mid: t50,
                    duration: t10 - t90,
                });
                state = State::Low;
                last_out = i;
            }
            _ => {}
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Square wave 0 V / 1 V, period 10 (samples every 0.01), duty 30%,
    /// with linear edges of duration 0.5 (0 to 100%).
    fn square() -> Vec<[f64; 2]> {
        (0..=4000)
            .map(|i| {
                let t = i as f64 * 0.01;
                let ph = t % 10.0;
                let v = if ph < 0.5 {
                    ph / 0.5
                } else if ph < 3.0 {
                    1.0
                } else if ph < 3.5 {
                    1.0 - (ph - 3.0) / 0.5
                } else {
                    0.0
                };
                [t, v]
            })
            .collect()
    }

    fn close(a: Option<f64>, b: f64, tol: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() <= tol)
    }

    #[test]
    fn square_wave() {
        let r = measure(&square(), &(0.0..=40.0), 0.01);
        let get = |m| r.get(m);
        assert!(close(get(Measurement::Vmax), 1.0, 1e-9));
        assert!(close(get(Measurement::Vmin), 0.0, 1e-9));
        assert!(close(get(Measurement::Vtop), 1.0, 0.01), "{r:?}");
        assert!(close(get(Measurement::Vbase), 0.0, 0.01), "{r:?}");
        assert!(close(get(Measurement::Period), 10.0, 0.01), "{r:?}");
        assert!(close(get(Measurement::Frequency), 0.1, 1e-3));
        // 10% to 90% of a 0.5 linear edge: 0.4.
        assert!(close(get(Measurement::RiseTime), 0.4, 0.02), "{r:?}");
        assert!(close(get(Measurement::FallTime), 0.4, 0.02), "{r:?}");
        // 50% points: rise at 0.25, fall at 3.25 → +width 3, −width 7.
        assert!(close(get(Measurement::PositiveWidth), 3.0, 0.02), "{r:?}");
        assert!(close(get(Measurement::NegativeWidth), 7.0, 0.02), "{r:?}");
        assert!(close(get(Measurement::DutyCycle), 30.0, 0.3), "{r:?}");
        // Mean: (2.5 + 2 × 0.25) / 10 = 0.3.
        assert!(close(get(Measurement::Mean), 0.3, 0.01), "{r:?}");
    }

    #[test]
    fn glitch_does_not_change_the_period() {
        // A spike from the top to the bottom in the middle of a high level.
        let mut pts = square();
        for p in pts.iter_mut().filter(|p| (p[0] - 11.5).abs() < 0.015) {
            p[1] = 0.0;
        }
        let r = measure(&pts, &(0.0..=40.0), 0.01);
        assert!(close(r.get(Measurement::Period), 10.0, 0.01), "{r:?}");
        assert!(close(r.get(Measurement::PositiveWidth), 3.0, 0.02), "{r:?}");
    }

    #[test]
    fn statistics_per_cycle() {
        let r = measure(&square(), &(0.0..=40.0), 0.01);
        let s = Stats::of(&r.per_cycle(Measurement::Period).unwrap()).unwrap();
        assert_eq!(s.count, 3, "4 rising edges: 3 whole cycles");
        assert!((s.mean - 10.0).abs() < 0.01 && s.std_dev < 0.01, "{s:?}");
        let d = Stats::of(&r.per_cycle(Measurement::DutyCycle).unwrap()).unwrap();
        assert!((d.mean - 30.0).abs() < 0.3, "{d:?}");
        assert_eq!(r.per_cycle(Measurement::Vpp), None);
        let s = Stats::of(&[1.0, 3.0]).unwrap();
        assert_eq!((s.mean, s.min, s.max, s.std_dev), (2.0, 1.0, 3.0, 1.0));
    }

    #[test]
    fn events_and_glitches() {
        let mut pts = square();
        for p in pts.iter_mut().filter(|p| (p[0] - 11.5).abs() < 0.015) {
            p[1] = 0.0;
        }
        let rising = find_events(&pts, EventKind::Rising, 0.01);
        assert_eq!(rising.len(), 4, "{rising:?}");
        assert!((rising[1] - 10.25).abs() < 0.02);
        let glitches = find_events(&pts, EventKind::Glitch, 0.01);
        assert_eq!(glitches.len(), 1, "{glitches:?}");
        assert!((glitches[0] - 11.5).abs() < 0.05);
        assert!(
            find_events(&pts, EventKind::Falling, 5.0).is_empty(),
            "noise guard"
        );
    }

    #[test]
    fn window_selects_samples() {
        // Only the low part: no edges, flat at 0 V.
        let r = measure(&square(), &(4.0..=9.0), 0.01);
        assert_eq!(r.get(Measurement::Vpp), Some(0.0));
        assert_eq!(r.get(Measurement::Frequency), None);
        assert_eq!(
            measure(&square(), &(100.0..=200.0), 0.01),
            Results::default()
        );
    }

    #[test]
    fn sine_rms_and_noise_guard() {
        let sine: Vec<[f64; 2]> = (0..10_000)
            .map(|i| {
                let t = i as f64 * 1e-3;
                [t, 2.0 * (std::f64::consts::TAU * t).sin()]
            })
            .collect();
        let r = measure(&sine, &(0.0..=10.0), 0.01);
        assert!(
            close(r.get(Measurement::Rms), 2.0 / 2f64.sqrt(), 1e-3),
            "{r:?}"
        );
        assert!(close(r.get(Measurement::Period), 1.0, 1e-3), "{r:?}");
        assert!(close(r.get(Measurement::DutyCycle), 50.0, 0.5), "{r:?}");
        // The amplitude (about 4 V) is below the guard: no timing values.
        let r = measure(&sine, &(0.0..=10.0), 5.0);
        assert_eq!(r.get(Measurement::Frequency), None);
        assert!(r.get(Measurement::Vpp).is_some());
    }

    #[test]
    fn settings_keep_display_order() {
        let mut s = Settings::default();
        s.set_selected(Measurement::Rms, true);
        s.set_selected(Measurement::Period, true);
        s.set_selected(Measurement::Vpp, false);
        let shown: Vec<_> = s.shown().collect();
        assert_eq!(
            shown,
            [
                Measurement::Frequency,
                Measurement::Period,
                Measurement::Rms
            ]
        );
    }
}
