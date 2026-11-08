//! I2C (two-wire bus: SDA data, SCL clock).
//!
//! - Start: SDA goes low while SCL is high. A start inside a transaction is
//!   a repeated start.
//! - Stop: SDA goes high while SCL is high.
//! - Bits: SDA is read on each rising edge of SCL, the most significant
//!   bit first. 8 bits, then the acknowledge bit (0 = ACK, 1 = NACK).
//! - The first byte after a start is the 7-bit address and the direction
//!   (bit 0: 1 = read). 10-bit addresses are shown as two bytes.

use super::{Decoded, Frame, FrameError, FrameKind, Logic};

/// Fewer samples per clock period than this: a warning.
const MIN_SAMPLES_PER_PERIOD: f64 = 4.0;

/// The clock period: the typical time between two rising edges of SCL
/// (the median of those up to twice the shortest one). `None` with fewer
/// than two rising edges.
pub fn clock_period(scl: &Logic) -> Option<f64> {
    let rising: Vec<f64> = rising_edges(scl).collect();
    let mut gaps: Vec<f64> = rising
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|g| *g > 0.0)
        .collect();
    let shortest = gaps.iter().copied().min_by(f64::total_cmp)?;
    gaps.retain(|g| *g <= 2.0 * shortest);
    gaps.sort_by(f64::total_cmp);
    Some(gaps[gaps.len() / 2])
}

fn rising_edges(l: &Logic) -> impl Iterator<Item = f64> + '_ {
    l.edges
        .iter()
        .enumerate()
        .filter(|(i, _)| l.level_after(*i))
        .map(|(_, &t)| t)
}

fn falling_edges(l: &Logic) -> impl Iterator<Item = f64> + '_ {
    l.edges
        .iter()
        .enumerate()
        .filter(|(i, _)| !l.level_after(*i))
        .map(|(_, &t)| t)
}

/// What happens on the bus, in time order.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Event {
    Start,
    Stop,
    /// A rising edge of SCL: a bit is read.
    Bit,
}

/// "100 kHz": a frequency in short.
fn freq_text(f: f64) -> String {
    if f >= 1e6 {
        format!("{:.3} MHz", f / 1e6)
    } else if f >= 1e3 {
        format!("{:.1} kHz", f / 1e3)
    } else {
        format!("{f:.0} Hz")
    }
}

/// Decode the I2C frames of `sda` and `scl`.
pub fn decode(sda: &Logic, scl: &Logic) -> Decoded {
    let mut warnings = Vec::new();
    let period = clock_period(scl);
    let title = match period {
        Some(p) => format!("SCL {}", freq_text(1.0 / p)),
        None => "SCL ?".to_owned(),
    };
    let half = period.map_or(0.0, |p| p / 2.0);
    if let Some(p) = period {
        let step = [sda.sample_step, scl.sample_step]
            .into_iter()
            .flatten()
            .fold(0.0, f64::max);
        if step > 0.0 && p / step < MIN_SAMPLES_PER_PERIOD {
            warnings.push(format!(
                "only {:.1} samples per clock period: the bytes may be wrong (use a faster time base)",
                p / step
            ));
        }
    } else {
        warnings.push("no clock: SCL does not change".to_owned());
    }

    // Start and stop: SDA changes while SCL is high. Bits: SCL rising.
    let mut events: Vec<(f64, Event)> = sda
        .edges
        .iter()
        .enumerate()
        .filter(|(_, t)| scl.level_at(**t) && !scl.edges.contains(t))
        .map(|(i, &t)| {
            (
                t,
                if sda.level_after(i) {
                    Event::Stop
                } else {
                    Event::Start
                },
            )
        })
        .collect();
    events.extend(rising_edges(scl).map(|t| (t, Event::Bit)));
    events.sort_by(|a, b| a.0.total_cmp(&b.0));

    let falls: Vec<f64> = falling_edges(scl).collect();
    let rises: Vec<f64> = rising_edges(scl).collect();
    // The SCL falling edge after `t` (at most half a period later).
    let fall_after = |t: f64| {
        let i = falls.partition_point(|&f| f <= t);
        falls
            .get(i)
            .copied()
            .filter(|f| f - t <= half.max(0.0) * 1.5)
            .unwrap_or(t + half)
    };
    // The SCL falling edge before `t` (at most half a period earlier).
    let fall_before = |t: f64| {
        let i = falls.partition_point(|&f| f < t);
        i.checked_sub(1)
            .map(|i| falls[i])
            .filter(|f| t - f <= half * 1.5)
            .unwrap_or(t - half)
    };
    let rise_before = |t: f64| {
        let i = rises.partition_point(|&r| r < t);
        i.checked_sub(1)
            .map(|i| rises[i])
            .filter(|r| t - r <= half * 1.5)
            .unwrap_or(t - half / 2.0)
    };

    let mut frames = Vec::new();
    let mut in_transaction = false;
    let mut first_byte = false;
    let mut bits: Vec<(f64, bool)> = Vec::new();
    // A start or stop at `at` ends the byte being read. The rising edge of
    // SCL just before a repeated start or a stop is part of that condition,
    // not a bit: it is left out.
    let cut = |bits: &mut Vec<(f64, bool)>, frames: &mut Vec<Frame>, at: f64| {
        if bits.last().is_some_and(|(t, _)| at - t <= half * 1.5) {
            bits.pop();
        }
        if let Some(&(first, _)) = bits.first() {
            let value = bits
                .iter()
                .take(8)
                .fold(0u32, |v, (_, b)| v << 1 | u32::from(*b));
            frames.push(Frame {
                kind: FrameKind::Data,
                start: fall_before(first),
                end: at,
                value,
                bits: bits.len().min(8) as u8,
                ack: None,
                error: Some(FrameError::Incomplete),
            });
        }
        bits.clear();
    };
    for (t, event) in events {
        match event {
            Event::Start => {
                cut(&mut bits, &mut frames, t);
                frames.push(Frame {
                    kind: FrameKind::Start {
                        repeated: in_transaction,
                    },
                    start: t,
                    end: fall_after(t).max(t),
                    value: 0,
                    bits: 0,
                    ack: None,
                    error: None,
                });
                in_transaction = true;
                first_byte = true;
            }
            Event::Stop => {
                cut(&mut bits, &mut frames, t);
                if in_transaction {
                    frames.push(Frame {
                        kind: FrameKind::Stop,
                        start: rise_before(t).min(t),
                        end: t,
                        value: 0,
                        bits: 0,
                        ack: None,
                        error: None,
                    });
                }
                in_transaction = false;
            }
            Event::Bit => {
                if !in_transaction {
                    continue;
                }
                bits.push((t, sda.level_at(t)));
                if bits.len() < 9 {
                    continue;
                }
                let byte = bits[..8]
                    .iter()
                    .fold(0u32, |v, (_, b)| v << 1 | u32::from(*b));
                let ack = !bits[8].1;
                let (kind, value, n) = if first_byte {
                    (
                        FrameKind::Address {
                            read: byte & 1 == 1,
                        },
                        byte >> 1,
                        7,
                    )
                } else {
                    (FrameKind::Data, byte, 8)
                };
                frames.push(Frame {
                    kind,
                    start: fall_before(bits[0].0),
                    end: fall_after(t),
                    value,
                    bits: n,
                    ack: Some(ack),
                    error: None,
                });
                first_byte = false;
                bits.clear();
            }
        }
    }
    Decoded {
        title,
        frames,
        warnings,
        threshold: 0.0,
        high: 0.0,
        low: 0.0,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// SDA and SCL of a bus, built step by step. One clock period is 4
    /// time units: SCL low for 2, high for 2; SDA changes while SCL is low.
    pub struct Bus {
        sda: Vec<bool>,
        scl: Vec<bool>,
    }

    impl Bus {
        pub fn new() -> Self {
            Self {
                sda: vec![true; 4],
                scl: vec![true; 4],
            }
        }

        fn push(&mut self, sda: bool, scl: bool) {
            self.sda.push(sda);
            self.scl.push(scl);
        }

        pub fn start(&mut self) {
            // SDA high, SCL high, then SDA low, then SCL low.
            let sda = *self.sda.last().unwrap();
            if !sda || !*self.scl.last().unwrap() {
                // Repeated start: release SDA while SCL is low, then raise SCL.
                self.push(true, false);
                self.push(true, true);
            }
            self.push(false, true);
            self.push(false, false);
        }

        pub fn stop(&mut self) {
            self.push(false, false);
            self.push(false, true);
            self.push(true, true);
            self.push(true, true);
        }

        pub fn bit(&mut self, b: bool) {
            self.push(b, false);
            self.push(b, false);
            self.push(b, true);
            self.push(b, true);
            self.push(b, false);
        }

        pub fn byte(&mut self, v: u8, ack: bool) {
            for k in (0..8).rev() {
                self.bit(v >> k & 1 == 1);
            }
            self.bit(!ack);
        }

        pub fn logic(&self) -> (Logic, Logic) {
            let make = |l: &[bool]| Logic {
                start: 0.0,
                end: l.len() as f64,
                initial: l[0],
                edges: l
                    .windows(2)
                    .enumerate()
                    .filter(|(_, w)| w[0] != w[1])
                    .map(|(i, _)| (i + 1) as f64)
                    .collect(),
                sample_step: Some(1.0),
            };
            (make(&self.sda), make(&self.scl))
        }
    }

    fn kinds(d: &Decoded) -> Vec<String> {
        d.frames
            .iter()
            .map(|f| match f.kind {
                FrameKind::Start { repeated: false } => "S".to_owned(),
                FrameKind::Start { repeated: true } => "Sr".to_owned(),
                FrameKind::Stop => "P".to_owned(),
                FrameKind::Select { .. } => "CS".to_owned(),
                FrameKind::Address { read } => format!(
                    "{}{:02X}{}",
                    if read { "R" } else { "W" },
                    f.value,
                    if f.ack == Some(true) { "" } else { "N" }
                ),
                FrameKind::Data => format!(
                    "{:02X}{}{}",
                    f.value,
                    if f.ack == Some(false) { "N" } else { "" },
                    if f.error.is_some() { "!" } else { "" }
                ),
            })
            .collect()
    }

    #[test]
    fn write_then_read_with_repeated_start() {
        let mut b = Bus::new();
        // Write register 0x00 of device 0x48, then read 2 bytes.
        b.start();
        b.byte(0x48 << 1, true);
        b.byte(0x00, true);
        b.start();
        b.byte(0x48 << 1 | 1, true);
        b.byte(0x19, true);
        b.byte(0x80, false);
        b.stop();
        let (sda, scl) = b.logic();
        let d = decode(&sda, &scl);
        assert_eq!(kinds(&d), ["S", "W48", "00", "Sr", "R48", "19", "80N", "P"]);
        assert!(d.warnings.is_empty(), "{:?}", d.warnings);
        // One clock period: 5 units (two low, two high, one low).
        assert_eq!(clock_period(&scl), Some(5.0));
        // Frames are in time order and do not overlap.
        for w in d.frames.windows(2) {
            assert!(
                w[0].start <= w[0].end && w[0].end <= w[1].start + 1e-9,
                "{w:?}"
            );
        }
    }

    #[test]
    fn address_without_answer_is_a_nack() {
        let mut b = Bus::new();
        b.start();
        b.byte(0x50 << 1, false);
        b.stop();
        let (sda, scl) = b.logic();
        assert_eq!(kinds(&decode(&sda, &scl)), ["S", "W50N", "P"]);
    }

    #[test]
    fn a_byte_cut_by_a_stop_is_incomplete() {
        let mut b = Bus::new();
        b.start();
        b.byte(0x48 << 1, true);
        for _ in 0..3 {
            b.bit(true);
        }
        b.stop();
        let (sda, scl) = b.logic();
        let d = decode(&sda, &scl);
        assert_eq!(kinds(&d), ["S", "W48", "07!", "P"]);
        assert_eq!(d.frames[2].error, Some(FrameError::Incomplete));
    }

    #[test]
    fn bits_outside_a_transaction_are_ignored() {
        let mut b = Bus::new();
        b.byte(0xAA, true);
        let (sda, scl) = b.logic();
        assert!(decode(&sda, &scl).frames.is_empty());
    }
}
