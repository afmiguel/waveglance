//! The internal data model: a capture from any oscilloscope, made for display
//! and analysis. The format readers (`crate::formats`) convert their files to
//! this model; the rest of the app knows nothing about file formats.
//!
//! - [`Capture`]: one file: where it comes from, its traces, the time base,
//!   the trigger and the acquisition.
//! - [`Trace`]: one channel (or a math or CSV column): samples in physical
//!   units (volts at the probe tip, or amperes), plus how the scope showed it.

use std::cell::OnceCell;
use std::ops::RangeInclusive;

/// One decoded file.
#[derive(Debug, Clone, PartialEq)]
pub struct Capture {
    pub source: Source,
    /// The channels in the file, in display order (CH1, CH2, …).
    pub traces: Vec<Trace>,
    /// Seconds per horizontal division, if the file stores it.
    pub time_per_div: Option<f64>,
    /// Trigger settings, if the file stores them.
    pub trigger: Option<Trigger>,
    pub acquisition: Acquisition,
    /// Samples per channel stored in the file (it can be more than a trace
    /// shows, for example all the frames of an equivalent-time record).
    pub record_length: usize,
    /// Non-fatal problems found while reading.
    pub warnings: Vec<String>,
}

impl Capture {
    /// The trace with label `label` ("CH1", …).
    pub fn trace(&self, label: &str) -> Option<&Trace> {
        self.traces.iter().find(|t| t.label == label)
    }

    /// The time span of all the traces.
    pub fn time_span(&self) -> Option<RangeInclusive<f64>> {
        let (lo, hi) = self
            .traces
            .iter()
            .filter_map(|t| t.time_span())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), r| {
                (lo.min(*r.start()), hi.max(*r.end()))
            });
        (lo <= hi).then_some(lo..=hi)
    }
}

/// Where a capture comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// Maker, for example "FNIRSI".
    pub vendor: String,
    /// Model, for example "DPOX180H" (empty if unknown).
    pub model: String,
    /// File format, for example "FNIRSI DPOX180H .wav".
    pub format: String,
}

impl Source {
    /// "FNIRSI DPOX180H": maker and model, for the title.
    pub fn title(&self) -> String {
        if self.model.is_empty() {
            self.vendor.clone()
        } else {
            format!("{} {}", self.vendor, self.model)
        }
    }
}

/// What kind of trace this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TraceId {
    /// An analog input channel: 1, 2, 3, 4…
    Analog(u8),
    /// A math trace computed from other traces.
    Math,
    /// A digital (logic) signal, 0 or 1: for example from a VCD file.
    Digital(u16),
}

/// Physical unit of a trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Unit {
    #[default]
    Volt,
    Ampere,
    /// Logic level: 0 or 1 (0.5 for an unknown or high-impedance value).
    Logic,
}

impl Unit {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Volt => "V",
            Self::Ampere => "A",
            Self::Logic => "",
        }
    }
}

/// Input coupling of a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coupling {
    Dc,
    Ac,
}

impl Coupling {
    pub fn name(self) -> &'static str {
        match self {
            Self::Dc => "DC",
            Self::Ac => "AC",
        }
    }
}

/// The samples of a trace.
#[derive(Debug, Clone, PartialEq)]
pub enum Samples {
    /// Evenly spaced: sample `i` is at time `t0 + i·dt`. Values in the unit of
    /// the trace (4 bytes per sample).
    Uniform { t0: f64, dt: f64, values: Vec<f32> },
    /// `[time_s, value]` pairs, for files with uneven times (some CSV files).
    Timed(Vec<[f64; 2]>),
}

impl Samples {
    pub fn len(&self) -> usize {
        match self {
            Self::Uniform { values, .. } => values.len(),
            Self::Timed(points) => points.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// How the scope showed a trace (the settings on its screen).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    /// Units per vertical division, as shown (includes the probe factor).
    pub per_div: f64,
    /// Position of 0 V (the ground marker) in divisions from the screen
    /// centre; up is positive.
    pub zero_div: f64,
    /// Probe attenuation (1.0 = 1X, 10.0 = 10X).
    pub probe: f64,
    pub coupling: Option<Coupling>,
}

/// One channel of a capture.
#[derive(Debug, Clone, PartialEq)]
pub struct Trace {
    pub id: TraceId,
    /// Name shown in the app: "CH1", "CH2", …
    pub label: String,
    pub unit: Unit,
    pub samples: Samples,
    pub display: Display,
    /// Smallest step between two values (one ADC code), if known.
    pub resolution: Option<f64>,
    /// Limits of the input range. A sample at a limit is clipped: the real
    /// signal can be beyond it.
    pub range: Option<(f64, f64)>,
    /// Smallest time step the file can express, if known (the `$timescale`
    /// of a VCD file).
    pub time_resolution: Option<f64>,
    /// `[time, value]` pairs, made from the samples when first needed.
    points: OnceCell<Vec<[f64; 2]>>,
}

impl Trace {
    pub fn new(id: TraceId, label: impl Into<String>, samples: Samples, display: Display) -> Self {
        Self {
            id,
            label: label.into(),
            unit: Unit::Volt,
            samples,
            display,
            resolution: None,
            range: None,
            time_resolution: None,
            points: OnceCell::new(),
        }
    }

    /// A digital (logic) trace: values 0 and 1, drawn as steps in a lane.
    pub fn is_digital(&self) -> bool {
        matches!(self.id, TraceId::Digital(_))
    }

    /// The samples as `[time_s, value]` pairs (made once, then kept).
    pub fn points(&self) -> &[[f64; 2]] {
        self.points.get_or_init(|| match &self.samples {
            Samples::Uniform { t0, dt, values } => values
                .iter()
                .enumerate()
                .map(|(i, &v)| [t0 + i as f64 * dt, f64::from(v)])
                .collect(),
            Samples::Timed(points) => points.clone(),
        })
    }

    /// Time between two samples (the first two for uneven times).
    pub fn sample_interval(&self) -> Option<f64> {
        match &self.samples {
            Samples::Uniform { dt, .. } => Some(*dt),
            Samples::Timed(p) => p.get(1).map(|b| b[0] - p[0][0]),
        }
    }

    /// Time resolution for readouts: the time resolution of the file if
    /// known, else the sample interval, else (uneven times) the smallest
    /// time between two points.
    pub fn time_step(&self) -> Option<f64> {
        if let Some(r) = self.time_resolution {
            return Some(r);
        }
        match &self.samples {
            Samples::Uniform { dt, .. } => Some(dt.abs()),
            Samples::Timed(p) => p
                .windows(2)
                .map(|w| w[1][0] - w[0][0])
                .filter(|d| *d > 0.0)
                .min_by(f64::total_cmp),
        }
    }

    /// Smallest step between two values: the resolution if known, else 1/100
    /// of a division.
    pub fn lsb(&self) -> f64 {
        self.resolution.unwrap_or(self.display.per_div / 100.0)
    }

    pub fn time_span(&self) -> Option<RangeInclusive<f64>> {
        let p = self.points();
        Some(p.first()?[0]..=p.last()?[0])
    }

    /// True if a sample with a time inside `window` is at a limit of the
    /// input range.
    pub fn clipped_in(&self, window: &RangeInclusive<f64>) -> bool {
        let Some((lo, hi)) = self.range else {
            return false;
        };
        // Half a code of margin: values are stored as f32.
        let eps = self.lsb() * 0.5;
        self.points()
            .iter()
            .any(|p| window.contains(&p[0]) && (p[1] <= lo + eps || p[1] >= hi - eps))
    }

    /// True if any sample is at a limit of the input range.
    pub fn clipped(&self) -> bool {
        self.clipped_in(&(f64::NEG_INFINITY..=f64::INFINITY))
    }
}

/// Trigger settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// "Auto", "Normal", "Single", …
    pub mode: String,
    /// True for the rising edge.
    pub rising: bool,
    /// Label of the source trace ("CH1"), if known.
    pub source: Option<String>,
}

/// How the scope acquired the samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Acquisition {
    /// Real-time sampling at `sample_rate_hz`.
    RealTime { sample_rate_hz: f64 },
    /// Equivalent-time sampling: the record holds `frames` screen frames;
    /// the traces show the first one.
    Equivalent { frames: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trace(values: Vec<f32>) -> Trace {
        let mut t = Trace::new(
            TraceId::Analog(1),
            "CH1",
            Samples::Uniform {
                t0: -1.0,
                dt: 0.5,
                values,
            },
            Display {
                per_div: 1.0,
                zero_div: 0.0,
                probe: 1.0,
                coupling: None,
            },
        );
        t.resolution = Some(0.01);
        t.range = Some((-4.0, 4.0));
        t
    }

    #[test]
    fn uniform_points_and_clipping() {
        let t = trace(vec![0.0, 1.0, 4.0, -2.0]);
        assert_eq!(t.points()[2], [0.0, 4.0]);
        assert_eq!(t.time_span(), Some(-1.0..=0.5));
        assert_eq!(t.sample_interval(), Some(0.5));
        assert!(t.clipped());
        assert!(t.clipped_in(&(-0.1..=0.1)), "the 4.0 sample at t = 0");
        assert!(!t.clipped_in(&(0.4..=0.6)));
    }
}
