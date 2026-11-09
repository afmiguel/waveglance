//! Protocol decoders (logic analyzer functions): serial data in one or
//! more traces made into bytes. Protocols: UART (one line), I2C (SDA and
//! SCL) and SPI (a data line, SCLK and an optional CS).
//!
//! The part shared by all protocols is here:
//!
//! - [`Logic`]: a trace as a logic signal (0 or 1): its first level and the
//!   times where it changes. An analog trace is compared with a threshold
//!   (with hysteresis); a digital trace (VCD) is used as it is.
//! - [`Frame`]: one decoded word (start and end time, kind, value, error).
//!
//! Each protocol is a submodule that makes frames from one or more
//! [`Logic`] signals. To add a protocol: add a submodule, a variant to
//! [`Protocol`] and its settings to [`Settings`].

pub mod i2c;
pub mod spi;
pub mod uart;

use crate::model::Trace;

/// A trace as a logic signal: the level at the start and the times where
/// the level changes (sorted). Level after `edges[i]` = `initial` changed
/// `i + 1` times.
#[derive(Debug, Clone, PartialEq)]
pub struct Logic {
    /// Time of the first sample.
    pub start: f64,
    /// Time of the last sample.
    pub end: f64,
    /// Level at `start`.
    pub initial: bool,
    pub edges: Vec<f64>,
    /// Time between two samples (the smallest, for uneven times), if known.
    pub sample_step: Option<f64>,
}

impl Logic {
    /// The level at time `t` (the level at `start` for an earlier time).
    pub fn level_at(&self, t: f64) -> bool {
        let n = self.edges.partition_point(|&e| e <= t);
        self.initial ^ (n % 2 == 1)
    }

    /// The level just after edge `i`.
    pub fn level_after(&self, i: usize) -> bool {
        self.initial ^ i.is_multiple_of(2)
    }

    /// The first edge at or after time `t`.
    pub fn next_edge(&self, t: f64) -> Option<usize> {
        let i = self.edges.partition_point(|&e| e < t);
        (i < self.edges.len()).then_some(i)
    }

    /// The time between each two edges that follow each other (the pulse
    /// widths).
    pub fn pulse_widths(&self) -> impl Iterator<Item = f64> + '_ {
        self.edges.windows(2).map(|w| w[1] - w[0])
    }
}

/// The threshold used for a trace: (level, hysteresis), in the unit of the
/// trace. Digital traces: 0.5 (a 0.5 value, unknown or high impedance,
/// keeps the last level). Analog: `manual`, or else the middle between
/// the lowest and highest sample; the hysteresis is 10 % of the swing.
pub fn threshold(trace: &Trace, manual: Option<f64>) -> (f64, f64) {
    if trace.is_digital() {
        return (0.5, 0.25);
    }
    let (lo, hi) = trace
        .points()
        .iter()
        .filter(|p| p[1].is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p[1]), hi.max(p[1]))
        });
    if lo > hi {
        return (manual.unwrap_or(0.0), 0.0);
    }
    let swing = hi - lo;
    (manual.unwrap_or((lo + hi) / 2.0), swing * 0.1)
}

/// The trace as a logic signal: a sample at or above `level + hysteresis/2`
/// is 1, at or below `level − hysteresis/2` is 0; in between, the level does
/// not change. The time of an analog edge is where the line between two
/// samples crosses `level`. `None` for an empty trace.
pub fn to_logic(trace: &Trace, level: f64, hysteresis: f64) -> Option<Logic> {
    let points = trace.points();
    let first = points.first()?;
    let last = points.last()?;
    let (high, low) = (level + hysteresis / 2.0, level - hysteresis / 2.0);
    let mut state = first[1] >= level;
    let initial = state;
    let mut edges = Vec::new();
    let digital = trace.is_digital();
    for w in points.windows(2) {
        let [a, b] = [w[0], w[1]];
        let next = if b[1] >= high {
            true
        } else if b[1] <= low {
            false
        } else {
            continue;
        };
        if next == state {
            continue;
        }
        state = next;
        // Digital steps have two points at the edge time; analog: where the
        // line crosses the threshold.
        let t = if digital || b[1] == a[1] {
            b[0]
        } else {
            let f = ((level - a[1]) / (b[1] - a[1])).clamp(0.0, 1.0);
            a[0] + f * (b[0] - a[0])
        };
        edges.push(t);
    }
    Some(Logic {
        start: first[0],
        end: last[0],
        initial,
        edges,
        sample_step: trace.time_step(),
    })
}

/// What is wrong with a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// The parity bit does not agree with the data bits.
    Parity,
    /// The stop bit is 0 (wrong baud rate, or a break).
    Framing,
    /// A byte cut by a start or stop condition (I2C).
    Incomplete,
}

impl FrameError {
    pub fn label(self) -> &'static str {
        match self {
            Self::Parity => "parity error",
            Self::Framing => "framing error (stop bit is 0)",
            Self::Incomplete => "incomplete byte (cut by a start or stop)",
        }
    }
}

/// What a frame is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameKind {
    /// A data word (UART character, I2C data byte).
    #[default]
    Data,
    /// I2C start condition (`repeated`: a start inside a transaction).
    Start { repeated: bool },
    /// I2C stop condition.
    Stop,
    /// I2C address byte: 7-bit address in `value`, and the direction.
    Address { read: bool },
    /// SPI chip select: active (the start of a selection) or released.
    Select { active: bool },
}

/// One decoded word.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub kind: FrameKind,
    /// Time of the start of the frame (start bit edge for UART).
    pub start: f64,
    /// Time of the end of the frame (end of the stop bits for UART).
    pub end: f64,
    pub value: u32,
    /// Number of data bits.
    pub bits: u8,
    /// I2C: the acknowledge bit (true = ACK, SDA low); `None` for UART.
    pub ack: Option<bool>,
    pub error: Option<FrameError>,
}

/// The protocols.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum Protocol {
    #[default]
    Off,
    Uart,
    I2c,
    Spi,
}

impl Protocol {
    /// The protocols that a line can use.
    pub const ALL: [Self; 3] = [Self::Uart, Self::I2c, Self::Spi];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Uart => "UART",
            Self::I2c => "I2C",
            Self::Spi => "SPI",
        }
    }

    /// The protocol needs a clock line (I2C: SCL, SPI: SCLK).
    pub fn needs_clock(self) -> bool {
        matches!(self, Self::I2c | Self::Spi)
    }

    /// The protocol can use a chip select line (SPI: CS).
    pub fn uses_select(self) -> bool {
        self == Self::Spi
    }
}

/// Decoder settings (saved between sessions).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub protocol: Protocol,
    /// Threshold of an analog trace, in its unit; `None` = automatic.
    pub threshold: Option<f64>,
    pub uart: uart::Settings,
    pub spi: spi::Settings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            protocol: Protocol::Off,
            threshold: None,
            uart: uart::Settings::default(),
            spi: spi::Settings::default(),
        }
    }
}

/// The result of a decoder for one trace.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    /// "9600 8N1": the settings used (for UART: baud, data bits, parity,
    /// stop bits).
    pub title: String,
    pub frames: Vec<Frame>,
    /// Problems that make the result doubtful (too few samples per bit…).
    pub warnings: Vec<String>,
    /// The threshold used (in the unit of the trace).
    pub threshold: f64,
    /// The highest and lowest samples of the trace (the words are drawn
    /// above or below them).
    pub high: f64,
    pub low: f64,
}

/// Decode `trace` with `settings`; `clock` is the clock line of a protocol
/// that needs one (I2C: SCL, with `trace` the SDA line; SPI: SCLK), and
/// `select` the chip select line (SPI CS, optional). `None` when the
/// protocol is Off or the trace is empty.
pub fn decode(
    trace: &Trace,
    clock: Option<&Trace>,
    select: Option<&Trace>,
    settings: &Settings,
) -> Option<Decoded> {
    let logic_of = |t: &Trace| {
        let (level, hysteresis) = threshold(t, settings.threshold);
        to_logic(t, level, hysteresis)
    };
    let (level, hysteresis) = threshold(trace, settings.threshold);
    let logic = to_logic(trace, level, hysteresis)?;
    let mut out = match settings.protocol {
        Protocol::Off => return None,
        Protocol::Uart => uart::decode(&logic, &settings.uart),
        Protocol::I2c | Protocol::Spi => match clock.and_then(logic_of) {
            Some(clk) if settings.protocol == Protocol::I2c => i2c::decode(&logic, &clk),
            Some(clk) => {
                let cs = select.and_then(logic_of);
                spi::decode(&logic, &clk, cs.as_ref(), &settings.spi)
            }
            None => Decoded {
                title: "no clock line".to_owned(),
                frames: Vec::new(),
                warnings: vec![format!(
                    "choose the clock line ({}) in the ▸ settings",
                    if settings.protocol == Protocol::I2c {
                        "SCL"
                    } else {
                        "SCLK"
                    }
                )],
                threshold: 0.0,
                high: 0.0,
                low: 0.0,
            },
        },
    };
    out.threshold = level;
    (out.low, out.high) = trace
        .points()
        .iter()
        .map(|p| p[1])
        .filter(|v| v.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
    Some(out)
}

/// A character as text: printable ASCII as it is, else an escape.
pub fn ascii(value: u32) -> String {
    match value {
        0x0A => "\\n".to_owned(),
        0x0D => "\\r".to_owned(),
        0x09 => "\\t".to_owned(),
        0x00 => "\\0".to_owned(),
        0x20..=0x7E => char::from_u32(value).map_or_else(String::new, |c| c.to_string()),
        v if v <= 0xFF => format!("\\x{v:02X}"),
        v => format!("\\u{v:X}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Display, Samples, TraceId};

    pub(crate) fn analog(t0: f64, dt: f64, values: Vec<f32>) -> Trace {
        Trace::new(
            TraceId::Analog(1),
            "CH1",
            Samples::Uniform { t0, dt, values },
            Display {
                per_div: 1.0,
                zero_div: 0.0,
                probe: 1.0,
                coupling: None,
            },
        )
    }

    #[test]
    fn analog_to_logic_with_hysteresis() {
        // 0 V, noise near the threshold, 3.3 V, back to 0 V.
        let t = analog(0.0, 1.0, vec![0.0, 0.0, 1.7, 1.6, 3.3, 3.3, 1.6, 0.0]);
        let (level, h) = threshold(&t, None);
        assert!((level - 1.65).abs() < 1e-6);
        let l = to_logic(&t, level, h).unwrap();
        assert!(!l.initial);
        // 1.7 and 1.6 are inside the hysteresis band (1.485..1.815): no edge.
        assert_eq!(l.edges.len(), 2);
        // Up between t = 3 (1.6 V) and t = 4 (3.3 V), where 1.65 V is crossed.
        let f = (level - 1.6f32 as f64) / (3.3f32 as f64 - 1.6f32 as f64);
        assert!((l.edges[0] - (3.0 + f)).abs() < 1e-9);
        assert!(l.level_at(4.5));
        assert!(!l.level_at(7.0));
        assert_eq!(l.next_edge(4.0), Some(1));
    }

    #[test]
    fn ascii_text() {
        assert_eq!(ascii(u32::from(b'A')), "A");
        assert_eq!(ascii(10), "\\n");
        assert_eq!(ascii(0x1B), "\\x1B");
    }
}
