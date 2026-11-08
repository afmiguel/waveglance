//! UART (asynchronous serial: RS-232 levels after the transceiver, TTL
//! serial of microcontrollers).
//!
//! A frame: the line is idle at 1; a start bit (0); 5 to 9 data bits, the
//! least significant bit first; an optional parity bit; 1 or 2 stop bits
//! (1). Each bit is read at its middle, one bit time (1 / baud) apart,
//! counted from the falling edge of the start bit.

use super::{Decoded, Frame, FrameError, Logic};

/// Parity bit.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum Parity {
    #[default]
    None,
    Even,
    Odd,
}

impl Parity {
    pub const ALL: [Self; 3] = [Self::None, Self::Even, Self::Odd];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Even => "Even",
            Self::Odd => "Odd",
        }
    }

    /// The letter of the short form "8N1".
    fn letter(self) -> char {
        match self {
            Self::None => 'N',
            Self::Even => 'E',
            Self::Odd => 'O',
        }
    }
}

/// UART settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Bits per second; 0 = measured from the signal.
    pub baud: u32,
    /// Data bits, 5 to 9.
    pub data_bits: u8,
    pub parity: Parity,
    /// Stop bits, 1 or 2.
    pub stop_bits: u8,
    /// The line is 0 when idle (inverted, for example RS-232 levels before
    /// the transceiver).
    pub idle_low: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            baud: 0,
            data_bits: 8,
            parity: Parity::None,
            stop_bits: 1,
            idle_low: false,
        }
    }
}

/// Standard baud rates. A measured rate within 4 % of one of them is
/// rounded to it.
pub const STANDARD_BAUDS: [u32; 24] = [
    300, 600, 1200, 2400, 4800, 9600, 14400, 19200, 28800, 31250, 38400, 57600, 74880, 115200,
    128000, 230400, 250000, 460800, 500000, 921600, 1000000, 1500000, 2000000, 3000000,
];

/// Fewer samples per bit than this: a warning (the middle of a bit is not
/// found well).
const MIN_SAMPLES_PER_BIT: f64 = 4.0;

/// The baud rate measured from the pulse widths: the shortest pulse is
/// taken as one bit, then all pulses up to 12 bits long give the average
/// bit time. `None` with fewer than two edges.
pub fn measure_baud(logic: &Logic) -> Option<f64> {
    let shortest = logic
        .pulse_widths()
        .filter(|w| *w > 0.0)
        .min_by(f64::total_cmp)?;
    let (mut sum_w, mut sum_k) = (0.0, 0.0);
    for w in logic.pulse_widths() {
        let k = (w / shortest).round();
        if (1.0..=12.0).contains(&k) {
            sum_w += w;
            sum_k += k;
        }
    }
    let baud = sum_k / sum_w;
    let standard = STANDARD_BAUDS
        .iter()
        .map(|&b| f64::from(b))
        .find(|b| ((baud - b) / b).abs() < 0.04);
    Some(standard.unwrap_or(baud))
}

/// Decode the UART frames of `logic`.
pub fn decode(logic: &Logic, s: &Settings) -> Decoded {
    let data_bits = s.data_bits.clamp(5, 9);
    let stop_bits = s.stop_bits.clamp(1, 2);
    let parity_bits = u8::from(s.parity != Parity::None);
    let mut warnings = Vec::new();
    let baud = if s.baud > 0 {
        Some(f64::from(s.baud))
    } else {
        let b = measure_baud(logic);
        if let Some(b) = b
            && !STANDARD_BAUDS.contains(&(b as u32))
        {
            warnings.push(format!(
                "measured {b:.0} baud, not a standard rate: set the baud rate if the bytes are wrong"
            ));
        }
        b
    };
    let mut title = format!(
        "{}{} {}{}{}",
        baud.map_or("?".to_owned(), |b| format!("{b:.0}")),
        if s.baud == 0 { " (auto)" } else { "" },
        data_bits,
        s.parity.letter(),
        stop_bits
    );
    if s.idle_low {
        title.push_str(" inverted");
    }
    let Some(baud) = baud else {
        warnings.push("no edges: the signal does not change".to_owned());
        return Decoded {
            title,
            frames: Vec::new(),
            warnings,
            threshold: 0.0,
        };
    };
    let bit = 1.0 / baud;
    if let Some(step) = logic.sample_step
        && step > 0.0
        && bit / step < MIN_SAMPLES_PER_BIT
    {
        warnings.push(format!(
            "only {:.1} samples per bit: the bytes may be wrong (use a faster time base)",
            bit / step
        ));
    }

    // Logical level: 1 = idle (mark).
    let logical = |raw: bool| raw != s.idle_low;
    let frame_bits = 1 + u32::from(data_bits) + u32::from(parity_bits) + u32::from(stop_bits);
    let mut frames = Vec::new();
    let mut i = 0;
    while i < logic.edges.len() {
        let e = logic.edges[i];
        // A start bit begins with a change to 0.
        if logical(logic.level_after(i)) {
            i += 1;
            continue;
        }
        // The middle of the last stop bit must be in the record.
        if e + (f64::from(frame_bits) - 0.5) * bit > logic.end {
            break;
        }
        let sample = |k: u32| logical(logic.level_at(e + (f64::from(k) + 0.5) * bit));
        // A pulse shorter than half a bit is not a start bit.
        if sample(0) {
            i += 1;
            continue;
        }
        let mut value = 0u32;
        for b in 0..u32::from(data_bits) {
            if sample(1 + b) {
                value |= 1 << b;
            }
        }
        let after_data = 1 + u32::from(data_bits);
        let mut error = None;
        if s.parity != Parity::None {
            let ones = value.count_ones() + u32::from(sample(after_data));
            let ok = match s.parity {
                Parity::Even => ones.is_multiple_of(2),
                _ => ones % 2 == 1,
            };
            if !ok {
                error = Some(FrameError::Parity);
            }
        }
        let first_stop = after_data + u32::from(parity_bits);
        if (0..u32::from(stop_bits)).any(|k| !sample(first_stop + k)) {
            error = Some(FrameError::Framing);
        }
        frames.push(Frame {
            start: e,
            end: e + f64::from(frame_bits) * bit,
            value,
            bits: data_bits,
            error,
        });
        // The next start bit can begin after the middle of the first stop bit.
        match logic.next_edge(e + (f64::from(first_stop) + 0.5) * bit) {
            Some(next) => i = next,
            None => break,
        }
    }
    Decoded {
        title,
        frames,
        warnings,
        threshold: 0.0,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The levels of a UART line sending `bytes` (8N1 unless `bits`), one
    /// level per bit, with `idle` bits of idle before, between and after.
    pub fn line(bytes: &[u32], bits: u8, parity: Parity, stop: u8, idle: usize) -> Vec<bool> {
        let mut out = vec![true; idle];
        for &b in bytes {
            out.push(false);
            for k in 0..bits {
                out.push(b >> k & 1 == 1);
            }
            let ones = (b & ((1 << bits) - 1)).count_ones();
            match parity {
                Parity::None => {}
                Parity::Even => out.push(ones % 2 == 1),
                Parity::Odd => out.push(ones.is_multiple_of(2)),
            }
            out.extend(std::iter::repeat_n(true, usize::from(stop)));
            out.extend(std::iter::repeat_n(true, idle));
        }
        out
    }

    /// A logic signal from levels of `bit` seconds each, starting at t = 0.
    pub fn logic_of(levels: &[bool], bit: f64) -> Logic {
        let edges = levels
            .windows(2)
            .enumerate()
            .filter(|(_, w)| w[0] != w[1])
            .map(|(i, _)| (i + 1) as f64 * bit)
            .collect();
        Logic {
            start: 0.0,
            end: levels.len() as f64 * bit,
            initial: levels[0],
            edges,
            sample_step: Some(bit / 20.0),
        }
    }

    #[test]
    fn hello_at_9600_8n1_measured() {
        let text = b"Hi!\n";
        let bytes: Vec<u32> = text.iter().map(|&b| u32::from(b)).collect();
        // A bit time 1.5 % off 9600 baud: still 9600.
        let logic = logic_of(&line(&bytes, 8, Parity::None, 1, 3), 1.0 / 9600.0 * 1.015);
        assert_eq!(measure_baud(&logic), Some(9600.0));
        let d = decode(&logic, &Settings::default());
        assert_eq!(d.title, "9600 (auto) 8N1");
        let got: Vec<u32> = d.frames.iter().map(|f| f.value).collect();
        assert_eq!(got, bytes);
        assert!(d.frames.iter().all(|f| f.error.is_none()));
        assert!(d.warnings.is_empty(), "{:?}", d.warnings);
        // Frame 0: start bit at 3 bits, 10 bits long.
        let bit = 1.015 / 9600.0;
        assert!((d.frames[0].start - 3.0 * bit).abs() < 1e-12);
        // The frame is 10 bits at the rate used (9600).
        assert!((d.frames[0].end - d.frames[0].start - 10.0 / 9600.0).abs() < 1e-12);
    }

    #[test]
    fn back_to_back_frames() {
        // No idle between the bytes: the next start bit follows the stop bit.
        let bytes = [0x55, 0x00, 0xFF, 0x80];
        let mut levels = vec![true; 2];
        levels.extend(line(&bytes, 8, Parity::None, 1, 0));
        let logic = logic_of(&levels, 1e-3);
        let s = Settings {
            baud: 1000,
            ..Settings::default()
        };
        let got: Vec<u32> = decode(&logic, &s).frames.iter().map(|f| f.value).collect();
        assert_eq!(got, bytes);
    }

    #[test]
    fn parity_and_seven_bits() {
        let bytes = [0x41, 0x42, 0x43];
        let mut levels = line(&bytes, 7, Parity::Even, 2, 2);
        let s = Settings {
            baud: 1000,
            data_bits: 7,
            parity: Parity::Even,
            stop_bits: 2,
            idle_low: false,
        };
        let d = decode(&logic_of(&levels, 1e-3), &s);
        assert_eq!(d.title, "1000 7E2");
        assert_eq!(d.frames.len(), 3);
        assert!(d.frames.iter().all(|f| f.error.is_none()));
        // Flip the parity bit of the second byte: idle 2 + frame 11 + idle 2,
        // then start + 7 data bits.
        let p = 2 + 11 + 2 + 8;
        levels[p] = !levels[p];
        let d = decode(&logic_of(&levels, 1e-3), &s);
        assert_eq!(d.frames[1].error, Some(FrameError::Parity));
        assert_eq!(d.frames[1].value, 0x42);
    }

    #[test]
    fn wrong_baud_gives_framing_errors() {
        let bytes = [0x00, 0x00];
        let logic = logic_of(&line(&bytes, 8, Parity::None, 1, 4), 1e-3);
        // Twice the real rate: the "stop bit" falls in the zeros.
        let s = Settings {
            baud: 2000,
            ..Settings::default()
        };
        let d = decode(&logic, &s);
        assert!(
            d.frames
                .iter()
                .any(|f| f.error == Some(FrameError::Framing))
        );
    }

    #[test]
    fn inverted_line() {
        let levels: Vec<bool> = line(&[0x31, 0x32], 8, Parity::None, 1, 2)
            .into_iter()
            .map(|l| !l)
            .collect();
        let s = Settings {
            baud: 1000,
            idle_low: true,
            ..Settings::default()
        };
        let got: Vec<u32> = decode(&logic_of(&levels, 1e-3), &s)
            .frames
            .iter()
            .map(|f| f.value)
            .collect();
        assert_eq!(got, [0x31, 0x32]);
    }

    #[test]
    fn a_frame_cut_by_the_record_end_is_left_out() {
        let mut levels = line(&[0x10, 0x20], 8, Parity::None, 1, 2);
        levels.truncate(levels.len() - 6);
        let s = Settings {
            baud: 1000,
            ..Settings::default()
        };
        let d = decode(&logic_of(&levels, 1e-3), &s);
        assert_eq!(d.frames.len(), 1);
    }

    #[test]
    fn few_samples_per_bit_warns() {
        let mut logic = logic_of(&line(&[0x10], 8, Parity::None, 1, 2), 1e-3);
        logic.sample_step = Some(0.5e-3);
        let s = Settings {
            baud: 1000,
            ..Settings::default()
        };
        assert_eq!(decode(&logic, &s).warnings.len(), 1);
    }
}
