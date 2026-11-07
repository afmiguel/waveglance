//! Reader for the `.wav` files of the FNIRSI 1013D and 1014D tablet
//! oscilloscopes (original FNIRSI firmware).
//!
//! Like the DPOX180H file, it is NOT a RIFF/WAV audio file. The layout is
//! fixed, 15000 bytes, little-endian:
//!
//! ```text
//!     0 ..   999  header: settings and the measurements of the scope
//!  1000 ..  3999  CH1 buffer, 1500 x u16 (50 codes per division, 30 divisions)
//!  4000 ..  6999  CH2 buffer, 1500 x u16
//!  7000 ..  8499  CH1 screen points, 750 x u16 (screen row = 449 - buffer value)
//!  8500 ..  9999  CH2 screen points
//! 10000 .. 14999  zeros
//! ```
//!
//! Header bytes used here (CH2 = CH1 offset + 10):
//!
//! | Byte    | Field                                                        |
//! |---------|--------------------------------------------------------------|
//! | 2 / 12  | channel shown (0 = off)                                      |
//! | 4 / 14  | V/div index: 5, 2.5, 1, 0.5, 0.2, 0.1, 0.05 V (times the probe) |
//! | 8 / 18  | coupling: 0 = DC, 1 = AC                                     |
//! | 10 / 20 | probe: 10^n (1X, 10X, 100X)                                  |
//! | 22      | time/div index 0..29: 50 s ... 10 ns (1-2-5 steps)           |
//! | 26      | trigger mode: Auto, Single, Normal                           |
//! | 28      | trigger edge: 0 = rising, 1 = falling                        |
//! | 30      | trigger source: 0 = CH1, 1 = CH2                             |
//! | 84 / 86 | u16: buffer value of 0 V (CH1 / CH2)                         |
//!
//! Sources: Robert Tidey (github.com/roberttidey/FNISR1013DScope), W. Voigt
//! (zero level), and lexus2026HA/FNIRSI-2C53T-Decoder (header bytes checked
//! against 21 files and screenshots from a real 1013D).
//!
//! The trigger is at the centre of the buffer (sample 750): in the sample
//! files, the signal crosses its mid level within 2 samples of it.
//! Firmware by Peco or Atlan writes other layouts; such files are rejected.

use super::{AdcScale, Confidence, Format, ReadError, adc_trace};
use crate::model::{Acquisition, Capture, Coupling, Source, Trace, Trigger};

/// The 1013D/1014D `.wav` format (original firmware).
pub struct Fnirsi1013dFormat;

impl Format for Fnirsi1013dFormat {
    fn name(&self) -> &'static str {
        "FNIRSI 1013D/1014D (.wav)"
    }

    /// No signature: a fixed size, then header values within their tables.
    fn probe(&self, bytes: &[u8]) -> Confidence {
        if bytes.len() != FILE_SIZE {
            return Confidence::No;
        }
        let ok = |at: usize, len: usize| usize::from(bytes[at]) < len;
        if ok(22, TIME_PER_DIV.len()) && ok(4, VOLTS_PER_DIV.len()) && ok(14, VOLTS_PER_DIV.len()) {
            Confidence::Likely
        } else {
            Confidence::Maybe
        }
    }

    fn read(&self, bytes: &[u8]) -> Result<Capture, ReadError> {
        read(bytes).map_err(|e| ReadError(e.to_string()))
    }
}

/// Why a 1013D/1014D file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Not 15000 bytes.
    WrongSize(usize),
    /// A header value that the original firmware does not write.
    Unsupported(String),
    /// Both channels are off.
    NoChannels,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongSize(n) => write!(f, "a 1013D/1014D file has {FILE_SIZE} bytes, not {n}"),
            Self::Unsupported(e) => write!(
                f,
                "1013D/1014D file not supported: {e}. Only the original FNIRSI firmware is supported (not Peco or Atlan)"
            ),
            Self::NoChannels => {
                write!(f, "the file contains no channel data (CH1 and CH2 are off)")
            }
        }
    }
}

/// Size of every 1013D/1014D waveform file.
pub const FILE_SIZE: usize = 15_000;
/// Samples per channel in the buffer.
pub const BUFFER_LEN: usize = 1500;
/// Buffer codes per vertical division.
pub const COUNTS_PER_DIV: f64 = 50.0;
/// Samples per horizontal division.
pub const SAMPLES_PER_DIV: f64 = 50.0;
/// Highest buffer code (8 divisions of 50 codes; 0 is the lowest).
pub const CODE_MAX: u16 = 399;
/// Code at the vertical centre of the screen (4 of the 8 divisions).
pub const CENTER_CODE: f64 = 200.0;

const VOLTS_PER_DIV: [f64; 7] = [5.0, 2.5, 1.0, 0.5, 0.2, 0.1, 0.05];
const TIME_PER_DIV: [f64; 30] = [
    50.0, 20.0, 10.0, 5.0, 2.0, 1.0, 0.5, 0.2, 0.1, 50e-3, 20e-3, 10e-3, 5e-3, 2e-3, 1e-3, 500e-6,
    200e-6, 100e-6, 50e-6, 20e-6, 10e-6, 5e-6, 2e-6, 1e-6, 500e-9, 200e-9, 100e-9, 50e-9, 20e-9,
    10e-9,
];
const TRIGGER_MODES: [&str; 3] = ["Auto", "Single", "Normal"];

/// Read the bytes of a 1013D/1014D `.wav` file.
///
/// # Errors
/// Returns a [`ParseError`] if the file has the wrong size, a header value
/// that the original firmware does not write, or no channel on.
pub fn read(bytes: &[u8]) -> Result<Capture, ParseError> {
    if bytes.len() != FILE_SIZE {
        return Err(ParseError::WrongSize(bytes.len()));
    }
    let index = |at: usize, len: usize, what: &str| -> Result<usize, ParseError> {
        let v = usize::from(bytes[at]);
        if v < len {
            Ok(v)
        } else {
            Err(ParseError::Unsupported(format!(
                "unknown {what} value {v} at byte {at}"
            )))
        }
    };

    let time_per_div_s = TIME_PER_DIV[index(22, TIME_PER_DIV.len(), "time/div")?];
    let dt = time_per_div_s / SAMPLES_PER_DIV;
    let trigger = Trigger {
        mode: TRIGGER_MODES[index(26, TRIGGER_MODES.len(), "trigger mode")?].to_owned(),
        rising: index(28, 2, "trigger edge")? == 0,
        source: Some(format!("CH{}", index(30, 2, "trigger source")? + 1)),
    };

    let mut warnings = Vec::new();
    let mut channel = |c: usize| -> Result<Option<Trace>, ParseError> {
        let name = format!("CH{}", c + 1);
        let base = c * 10;
        let vdiv = VOLTS_PER_DIV[index(4 + base, VOLTS_PER_DIV.len(), &format!("{name} V/div"))?];
        let coupling = match index(8 + base, 2, &format!("{name} coupling"))? {
            0 => Coupling::Dc,
            _ => Coupling::Ac,
        };
        let probe_factor = 10f64.powi(index(10 + base, 3, &format!("{name} probe"))? as i32);
        if bytes[2 + base] == 0 {
            return Ok(None);
        }
        let start = 1000 + c * 3000;
        let raw: Vec<u16> = bytes[start..start + BUFFER_LEN * 2]
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        let zero = u16::from_le_bytes([bytes[84 + 2 * c], bytes[85 + 2 * c]]);
        if zero > CODE_MAX {
            warnings.push(format!("{name} zero level {zero} is outside the screen"));
        }
        let scale = AdcScale {
            per_div: vdiv * probe_factor,
            counts_per_div: COUNTS_PER_DIV,
            zero_code: f64::from(zero),
            center_code: CENTER_CODE,
            code_max: f64::from(CODE_MAX),
        };
        let t0 = -(BUFFER_LEN as f64 / 2.0) * dt;
        let ch = adc_trace(
            c as u8 + 1,
            &raw,
            scale,
            t0,
            dt,
            probe_factor,
            Some(coupling),
        );
        Ok(Some(ch))
    };
    let ch1 = channel(0)?;
    let ch2 = channel(1)?;
    if ch1.is_none() && ch2.is_none() {
        return Err(ParseError::NoChannels);
    }

    Ok(Capture {
        source: Source {
            vendor: "FNIRSI".into(),
            model: "1013D/1014D".into(),
            format: Fnirsi1013dFormat.name().into(),
        },
        traces: [ch1, ch2].into_iter().flatten().collect(),
        time_per_div: Some(time_per_div_s),
        trigger: Some(trigger),
        acquisition: Acquisition::RealTime {
            sample_rate_hz: 1.0 / dt,
        },
        record_length: BUFFER_LEN,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic file: CH1 on, 1 V/div, 1X, DC, 1 ms/div, zero at 200,
    /// a ramp of codes 100..=300.
    fn fake() -> Vec<u8> {
        let mut b = vec![0u8; FILE_SIZE];
        b[2] = 1;
        b[4] = 2; // 1 V/div
        b[22] = 14; // 1 ms/div
        b[84..86].copy_from_slice(&200u16.to_le_bytes());
        for i in 0..BUFFER_LEN {
            let code = 100 + (i * 200 / (BUFFER_LEN - 1)) as u16;
            b[1000 + 2 * i..1002 + 2 * i].copy_from_slice(&code.to_le_bytes());
        }
        b
    }

    #[test]
    fn decodes_settings_and_samples() {
        assert_eq!(Fnirsi1013dFormat.probe(&fake()), Confidence::Likely);
        let w = crate::formats::open(&fake()).unwrap();
        assert_eq!(w.source.model, "1013D/1014D");
        assert!(w.trace("CH2").is_none());
        assert!((w.time_per_div.unwrap() - 1e-3).abs() < 1e-15);
        let ch1 = w.trace("CH1").unwrap();
        assert_eq!(ch1.display.coupling, Some(Coupling::Dc));
        let p = ch1.points();
        assert_eq!(p.len(), BUFFER_LEN);
        // Code 100 is 2 divisions below zero: -2 V. Code 300: +2 V.
        assert!((p[0][1] + 2.0).abs() < 1e-6);
        assert!((p[BUFFER_LEN - 1][1] - 2.0).abs() < 1e-6);
        // 50 samples per division; trigger (t = 0) at sample 750.
        assert!((p[750][0]).abs() < 1e-15);
        assert!((p[800][0] - 1e-3).abs() < 1e-12);
        assert_eq!(
            w.trigger,
            Some(Trigger {
                mode: "Auto".into(),
                rising: true,
                source: Some("CH1".into())
            })
        );
    }

    #[test]
    fn probe_and_coupling() {
        let mut b = fake();
        b[8] = 1; // AC
        b[10] = 1; // 10X
        let w = read(&b).unwrap();
        let ch1 = w.trace("CH1").unwrap();
        assert_eq!(ch1.display.coupling, Some(Coupling::Ac));
        assert_eq!(ch1.display.probe, 10.0);
        assert!((ch1.display.per_div - 10.0).abs() < 1e-12);
        assert!((ch1.points()[0][1] + 20.0).abs() < 1e-5);
    }

    #[test]
    fn rejects_other_firmware_values() {
        let mut b = fake();
        b[22] = 77;
        assert_eq!(Fnirsi1013dFormat.probe(&b), Confidence::Maybe);
        assert!(matches!(read(&b), Err(ParseError::Unsupported(_))));
        let mut b = fake();
        b[2] = 0;
        assert_eq!(read(&b), Err(ParseError::NoChannels));
    }
}
