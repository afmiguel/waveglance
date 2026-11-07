//! File formats: finding the format of a file, and reading it into the
//! internal model ([`crate::model::Capture`]).
//!
//! Each format has its own module. A format says how sure it is that a file
//! is in its format ([`Format::probe`]), and reads it ([`Format::read`]).
//! [`open`] tries the formats from the most to the least sure.

pub mod csv;
pub mod fnirsi_1013d;
pub mod fnirsi_dpox;

use crate::model::{Capture, Display, Samples, Trace, TraceId};

/// How sure a format is that a file is in that format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    /// Not this format.
    No,
    /// Possible: for example only the size fits.
    Maybe,
    /// The structure fits, but there is no signature.
    Likely,
    /// A signature or a self-check fits.
    Sure,
}

/// A file format that the app can read.
pub trait Format: Sync {
    /// Name for the user: "FNIRSI DPOX180H (.wav)".
    fn name(&self) -> &'static str;
    /// How sure this format is that `bytes` is in this format. Only looks at
    /// the bytes; it does not read the whole file.
    fn probe(&self, bytes: &[u8]) -> Confidence;
    /// Read the file.
    ///
    /// # Errors
    /// A [`ReadError`] if the bytes are not a valid file of this format.
    fn read(&self, bytes: &[u8]) -> Result<Capture, ReadError>;
}

/// Why a file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadError(pub String);

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ReadError {}

/// All the formats, in the order they are tried when they are equally sure.
pub static FORMATS: &[&dyn Format] = &[
    &fnirsi_dpox::DpoxFormat,
    &fnirsi_1013d::Fnirsi1013dFormat,
    &csv::CsvFormat,
];

/// The formats that may fit `bytes`, the most sure first.
pub fn detect(bytes: &[u8]) -> Vec<(&'static dyn Format, Confidence)> {
    let mut found: Vec<(&'static dyn Format, Confidence)> = FORMATS
        .iter()
        .map(|&f| (f, f.probe(bytes)))
        .filter(|(_, c)| *c > Confidence::No)
        .collect();
    // Stable sort: equally sure formats keep their order in FORMATS.
    found.sort_by_key(|f| std::cmp::Reverse(f.1));
    found
}

/// What to do with a file, after detection.
pub enum Decision {
    /// One format is clearly the right one: read with it.
    Read(&'static dyn Format),
    /// Not clear: ask the user. The candidates, the most likely first (empty
    /// if no format recognises the file).
    Ask(Vec<(&'static dyn Format, Confidence)>),
}

/// Decide how to open `bytes`: read directly if one format is `Sure`, or is
/// the only `Likely` one; else ask the user.
pub fn decide(bytes: &[u8]) -> Decision {
    let found = detect(bytes);
    let top = found.first().map(|f| f.1);
    let likely = found.iter().filter(|f| f.1 == Confidence::Likely).count();
    match top {
        Some(Confidence::Sure) => Decision::Read(found[0].0),
        Some(Confidence::Likely) if likely == 1 => Decision::Read(found[0].0),
        _ => Decision::Ask(found),
    }
}

/// Find the format of `bytes` and read it. The formats are tried from the
/// most to the least sure; the first one that reads the file wins.
///
/// # Errors
/// A [`ReadError`]: the error of the most likely format, or "unknown file".
pub fn open(bytes: &[u8]) -> Result<Capture, ReadError> {
    let mut first_error = None;
    for (format, _) in detect(bytes) {
        match format.read(bytes) {
            Ok(capture) => return Ok(capture),
            Err(e) => {
                first_error.get_or_insert(e);
            }
        }
    }
    Err(first_error.unwrap_or_else(|| {
        let names: Vec<&str> = FORMATS.iter().map(|f| f.name()).collect();
        ReadError(format!(
            "unknown file ({} bytes): not a supported format ({})",
            bytes.len(),
            names.join(", ")
        ))
    }))
}

/// How ADC codes become volts, for formats that store codes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AdcScale {
    /// Volts per division, as shown (includes the probe).
    pub per_div: f64,
    /// ADC codes per division.
    pub counts_per_div: f64,
    /// Code of 0 V.
    pub zero_code: f64,
    /// Code at the vertical centre of the screen.
    pub center_code: f64,
    /// Highest code; codes 0 and `code_max` are at the input limits.
    pub code_max: f64,
}

impl AdcScale {
    pub fn volts(&self, code: f64) -> f64 {
        (code - self.zero_code) / self.counts_per_div * self.per_div
    }
}

/// An analog trace from ADC codes, evenly spaced: sample `i` at `t0 + i·dt`.
pub(crate) fn adc_trace(
    number: u8,
    codes: &[u16],
    scale: AdcScale,
    t0: f64,
    dt: f64,
    probe: f64,
    coupling: Option<crate::model::Coupling>,
) -> Trace {
    let values = codes
        .iter()
        .map(|&c| scale.volts(f64::from(c)) as f32)
        .collect();
    let mut trace = Trace::new(
        TraceId::Analog(number),
        format!("CH{number}"),
        Samples::Uniform { t0, dt, values },
        Display {
            per_div: scale.per_div,
            zero_div: (scale.zero_code - scale.center_code) / scale.counts_per_div,
            probe,
            coupling,
        },
    );
    trace.resolution = Some(scale.per_div / scale.counts_per_div);
    trace.range = Some((scale.volts(0.0), scale.volts(scale.code_max)));
    trace
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions() {
        assert!(matches!(decide(&[0; 10]), Decision::Ask(c) if c.is_empty()));
        // A 15000-byte file with impossible 1013D header values: only "Maybe".
        let mut b = vec![0xFFu8; 15_000];
        b[0] = 1;
        assert!(matches!(decide(&b), Decision::Ask(c) if c.len() == 1));
    }

    #[test]
    fn unknown_bytes() {
        let e = open(&[0; 10]).unwrap_err();
        assert!(e.0.starts_with("unknown file (10 bytes)"), "{e}");
        assert!(detect(&[0; 10]).is_empty());
    }
}
