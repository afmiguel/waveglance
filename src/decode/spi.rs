//! SPI (serial peripheral interface: SCLK clock, MOSI and MISO data, CS
//! chip select).
//!
//! One data line (MOSI or MISO) is decoded with the clock. Each bit is read
//! on the sampling edge of SCLK, which the mode gives:
//!
//! | Mode | CPOL (idle clock) | CPHA | Sampling edge |
//! |------|-------------------|------|---------------|
//! | 0    | low               | 0    | rising        |
//! | 1    | low               | 1    | falling       |
//! | 2    | high              | 0    | falling       |
//! | 3    | high              | 1    | rising        |
//!
//! With a CS line, the words are counted from the start of each selection
//! (CS active), and the selection is marked (active and released). Without
//! CS, a pause of the clock longer than `GAP_PERIODS` clock periods starts a
//! new word.

use super::{Decoded, Frame, FrameError, FrameKind, Logic};

/// SPI settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 0 to 3 (CPOL × 2 + CPHA).
    pub mode: u8,
    /// Most significant bit first (the usual order).
    pub msb_first: bool,
    /// Bits per word, 4 to 16.
    pub word_bits: u8,
    /// CS is active when low (the usual case).
    pub cs_active_low: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: 0,
            msb_first: true,
            word_bits: 8,
            cs_active_low: true,
        }
    }
}

/// Without CS: a pause of the clock longer than this many clock periods
/// starts a new word.
pub const GAP_PERIODS: f64 = 3.0;
/// Fewer samples per clock period than this: a warning.
const MIN_SAMPLES_PER_PERIOD: f64 = 4.0;

/// The times of the edges of `l` that go to `level`.
fn edges_to(l: &Logic, level: bool) -> Vec<f64> {
    l.edges
        .iter()
        .enumerate()
        .filter(|(i, _)| l.level_after(*i) == level)
        .map(|(_, &t)| t)
        .collect()
}

/// The typical time between two sampling edges (the median of those up to
/// twice the shortest one).
fn period(samples: &[f64]) -> Option<f64> {
    let mut gaps: Vec<f64> = samples
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|g| *g > 0.0)
        .collect();
    let shortest = gaps.iter().copied().min_by(f64::total_cmp)?;
    gaps.retain(|g| *g <= 2.0 * shortest);
    gaps.sort_by(f64::total_cmp);
    Some(gaps[gaps.len() / 2])
}

fn freq_text(f: f64) -> String {
    if f >= 1e6 {
        format!("{:.3} MHz", f / 1e6)
    } else if f >= 1e3 {
        format!("{:.1} kHz", f / 1e3)
    } else {
        format!("{f:.0} Hz")
    }
}

/// Decode the SPI words of `data` with clock `sclk` and chip select `cs`.
pub fn decode(data: &Logic, sclk: &Logic, cs: Option<&Logic>, s: &Settings) -> Decoded {
    let mode = s.mode.min(3);
    let word_bits = s.word_bits.clamp(4, 16);
    // Modes 0 and 3 sample on the rising edge, 1 and 2 on the falling edge.
    let sample_rising = mode == 0 || mode == 3;
    let samples = edges_to(sclk, sample_rising);
    let period = period(&samples);
    let half = period.map_or(0.0, |p| p / 2.0);
    let mut warnings = Vec::new();
    let mut title = format!(
        "SCLK {}, mode {mode}, {}, {word_bits} bits",
        period.map_or("?".to_owned(), |p| freq_text(1.0 / p)),
        if s.msb_first {
            "MSB first"
        } else {
            "LSB first"
        }
    );
    if cs.is_none() {
        title.push_str(", no CS");
    }
    match period {
        Some(p) => {
            let step = [data.sample_step, sclk.sample_step]
                .into_iter()
                .flatten()
                .fold(0.0, f64::max);
            if step > 0.0 && p / step < MIN_SAMPLES_PER_PERIOD {
                warnings.push(format!(
                    "only {:.1} samples per clock period: the words may be wrong (use a faster time base)",
                    p / step
                ));
            }
        }
        None => warnings.push("no clock: SCLK does not change".to_owned()),
    }

    // The selections: (start, end) times where CS is active. Without CS,
    // one selection over the whole record.
    let active = |level: bool| level != s.cs_active_low;
    let selections: Vec<(f64, f64)> = match cs {
        Some(cs) => {
            let mut out = Vec::new();
            let mut start = active(cs.initial).then_some(cs.start);
            for (i, &t) in cs.edges.iter().enumerate() {
                match (start, active(cs.level_after(i))) {
                    (None, true) => start = Some(t),
                    (Some(a), false) => {
                        out.push((a, t));
                        start = None;
                    }
                    _ => {}
                }
            }
            if let Some(a) = start {
                out.push((a, cs.end));
            }
            out
        }
        None => vec![(f64::NEG_INFINITY, f64::INFINITY)],
    };

    let mut frames = Vec::new();
    let push_word = |bits: &[(f64, bool)], frames: &mut Vec<Frame>, complete: bool| {
        let Some(&(first, _)) = bits.first() else {
            return;
        };
        let last = bits.last().map_or(first, |b| b.0);
        let value = if s.msb_first {
            bits.iter().fold(0u32, |v, (_, b)| v << 1 | u32::from(*b))
        } else {
            bits.iter()
                .enumerate()
                .fold(0u32, |v, (k, (_, b))| v | u32::from(*b) << k)
        };
        frames.push(Frame {
            kind: FrameKind::Data,
            start: first - half,
            end: last + half,
            value,
            bits: bits.len() as u8,
            ack: None,
            error: (!complete).then_some(FrameError::Incomplete),
        });
    };
    for (a, b) in selections {
        if cs.is_some() {
            frames.push(Frame {
                kind: FrameKind::Select { active: true },
                start: a,
                end: a,
                value: 0,
                bits: 0,
                ack: None,
                error: None,
            });
        }
        let from = samples.partition_point(|&t| t <= a);
        let to = samples.partition_point(|&t| t < b);
        let mut bits: Vec<(f64, bool)> = Vec::new();
        for &t in &samples[from..to] {
            // Without CS, a pause of the clock ends the word.
            if cs.is_none()
                && let (Some(&(prev, _)), Some(p)) = (bits.last(), period)
                && t - prev > GAP_PERIODS * p
            {
                push_word(&bits, &mut frames, false);
                bits.clear();
            }
            bits.push((t, data.level_at(t)));
            if bits.len() == usize::from(word_bits) {
                push_word(&bits, &mut frames, true);
                bits.clear();
            }
        }
        // Bits left when CS ends: an incomplete word.
        push_word(&bits, &mut frames, false);
        if cs.is_some() {
            frames.push(Frame {
                kind: FrameKind::Select { active: false },
                start: b,
                end: b,
                value: 0,
                bits: 0,
                ack: None,
                error: None,
            });
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
mod tests {
    use super::*;

    /// Lines of an SPI bus: one clock period is 4 time units. `words` are
    /// sent MSB first in `mode`, with CS low around them (if `with_cs`).
    fn bus(words: &[u32], bits: u8, mode: u8, with_cs: bool, gap: usize) -> (Logic, Logic, Logic) {
        let cpol = mode >= 2;
        let cpha = mode % 2 == 1;
        let (mut data, mut sclk, mut cs) = (vec![false; 4], vec![cpol; 4], vec![true; 4]);
        let mut push = |d: bool, c: bool, s: bool| {
            data.push(d);
            sclk.push(c);
            cs.push(s);
        };
        for (n, &w) in words.iter().enumerate() {
            if n > 0 {
                // Between words: CS inactive.
                for _ in 0..gap {
                    push(false, cpol, true);
                }
            }
            push(false, cpol, !with_cs);
            for k in (0..bits).rev() {
                let b = w >> k & 1 == 1;
                // CPHA 0: data before the first edge; CPHA 1: after it.
                if cpha {
                    push(false, !cpol, false);
                    push(b, !cpol, false);
                    push(b, cpol, false);
                    push(b, cpol, false);
                } else {
                    push(b, cpol, false);
                    push(b, !cpol, false);
                    push(b, !cpol, false);
                    push(b, cpol, false);
                }
            }
            push(false, cpol, !with_cs);
        }
        for _ in 0..4 {
            push(false, cpol, true);
        }
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
        (make(&data), make(&sclk), make(&cs))
    }

    fn values(d: &Decoded) -> Vec<u32> {
        d.frames
            .iter()
            .filter(|f| f.kind == FrameKind::Data)
            .map(|f| f.value)
            .collect()
    }

    #[test]
    fn all_four_modes() {
        let words = [0x9F, 0x00, 0xA5, 0xFF];
        for mode in 0..4 {
            let (data, sclk, cs) = bus(&words, 8, mode, true, 0);
            let s = Settings {
                mode,
                ..Settings::default()
            };
            let d = decode(&data, &sclk, Some(&cs), &s);
            assert_eq!(values(&d), words, "mode {mode}");
            assert!(d.warnings.is_empty(), "mode {mode}: {:?}", d.warnings);
            assert!(d.frames.iter().all(|f| f.error.is_none()), "mode {mode}");
        }
    }

    #[test]
    fn cs_marks_each_selection() {
        let (data, sclk, cs) = bus(&[0x12, 0x34], 8, 0, true, 6);
        let d = decode(&data, &sclk, Some(&cs), &Settings::default());
        let kinds: Vec<&str> = d
            .frames
            .iter()
            .map(|f| match f.kind {
                FrameKind::Select { active: true } => "S",
                FrameKind::Select { active: false } => "P",
                _ => "D",
            })
            .collect();
        assert_eq!(kinds, ["S", "D", "P", "S", "D", "P"]);
    }

    #[test]
    fn lsb_first_and_16_bit_words() {
        let (data, sclk, cs) = bus(&[0x1234], 16, 0, true, 0);
        let s = Settings {
            word_bits: 16,
            ..Settings::default()
        };
        assert_eq!(values(&decode(&data, &sclk, Some(&cs), &s)), [0x1234]);
        // The same bits read LSB first: reversed.
        let s = Settings {
            msb_first: false,
            ..s
        };
        assert_eq!(
            values(&decode(&data, &sclk, Some(&cs), &s)),
            [0x1234u32.reverse_bits() >> 16]
        );
    }

    #[test]
    fn without_cs_a_pause_of_the_clock_starts_a_new_word() {
        // Words separated by pauses; the decoder starts in the middle of a
        // word would otherwise be off forever.
        let (data, sclk, _) = bus(&[0x81, 0x42, 0x3C], 8, 0, false, 20);
        let d = decode(&data, &sclk, None, &Settings::default());
        assert_eq!(values(&d), [0x81, 0x42, 0x3C]);
        assert!(d.title.ends_with("no CS"));
    }

    #[test]
    fn a_word_cut_by_cs_is_incomplete() {
        let (data, sclk, mut cs) = bus(&[0xF0], 8, 0, true, 0);
        // CS goes inactive after 4 of the 8 bits (4 + 1 + 4 × 4 units).
        let rise = cs.edges.len() - 1;
        cs.edges[rise] = 4.0 + 1.0 + 16.0;
        let d = decode(&data, &sclk, Some(&cs), &Settings::default());
        let words: Vec<&Frame> = d
            .frames
            .iter()
            .filter(|f| f.kind == FrameKind::Data)
            .collect();
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].error, Some(FrameError::Incomplete));
        assert_eq!((words[0].value, words[0].bits), (0xF, 4));
    }
}
