//! File formats: finding the format of a file, and reading it into the
//! internal model ([`crate::model::Capture`]).
//!
//! Each format has its own module. A format says how sure it is that a file
//! is in its format ([`Format::probe`]), and reads it ([`Format::read`]).
//! [`open`] tries the formats from the most to the least sure.

pub mod csv;
pub mod digilent;
pub mod fnirsi_1013d;
pub mod fnirsi_dpox;
pub mod keysight_bin;
pub mod rigol_wfm;
pub mod vcd;

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
    &keysight_bin::KeysightBinFormat,
    &rigol_wfm::RigolWfmFormat,
    &vcd::VcdFormat,
    &digilent::DigilentFormat,
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
    let (bytes, cut) = within_limit(bytes)?;
    let mut first_error = None;
    for (format, _) in detect(bytes) {
        match format.read(bytes) {
            Ok(mut capture) => {
                if cut {
                    note_cut(&mut capture);
                }
                return Ok(capture);
            }
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

/// The largest file that is read (256 MiB). A text file (CSV, VCD) takes
/// several times its size in memory while it is read, and the web version
/// has a few GB at most.
pub const MAX_FILE_BYTES: usize = 256 * 1024 * 1024;

/// An error for a file larger than [`MAX_FILE_BYTES`].
///
/// # Errors
/// A [`ReadError`] that says the size and the limit.
pub fn check_size(len: usize) -> Result<(), ReadError> {
    if len > MAX_FILE_BYTES {
        return Err(ReadError(format!(
            "the file is too large ({} MB; the limit is {} MB)",
            len / (1024 * 1024),
            MAX_FILE_BYTES / (1024 * 1024)
        )));
    }
    Ok(())
}

/// Bytes looked at to tell a text file from a binary one.
const TEXT_PROBE: usize = 4096;

/// True if `head` (the start of a file) looks like text: no NUL bytes and
/// only a few control characters.
pub fn looks_like_text(head: &[u8]) -> bool {
    let head = &head[..head.len().min(TEXT_PROBE)];
    !head.is_empty()
        && !head.contains(&0)
        && head
            .iter()
            .filter(|&&b| b < 0x20 && !matches!(b, b'\n' | b'\r' | b'\t'))
            .count()
            * 100
            <= head.len()
}

/// The part of a file that is read. A file up to [`MAX_FILE_BYTES`] is
/// read whole. A larger text file (a long CSV or Digilent Record-mode
/// export, a VCD) is cut at the last full line before the limit, and the
/// second value is `true`: the start of the record is shown. A larger
/// binary file is refused.
///
/// # Errors
/// A [`ReadError`] for a binary file over the limit.
pub fn within_limit(bytes: &[u8]) -> Result<(&[u8], bool), ReadError> {
    if bytes.len() <= MAX_FILE_BYTES {
        return Ok((bytes, false));
    }
    if !looks_like_text(bytes) {
        check_size(bytes.len())?;
    }
    let head = &bytes[..MAX_FILE_BYTES];
    let end = head.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    if end == 0 {
        check_size(bytes.len())?;
    }
    Ok((&bytes[..end], true))
}

/// Tell, in the warnings of `capture`, that only the start of a large file
/// was read, and up to which time.
pub fn note_cut(capture: &mut Capture) {
    let until = capture
        .time_span()
        .map(|r| format!(" (up to t = {:.6} s)", r.end()))
        .unwrap_or_default();
    capture.warnings.insert(
        0,
        format!(
            "the file is larger than {} MB: only its first {} MB are shown{until}",
            MAX_FILE_BYTES / (1024 * 1024),
            MAX_FILE_BYTES / (1024 * 1024)
        ),
    );
}

/// A nice V/div in the 1-2-5 sequence, for formats that do not store it:
/// about `span` / 6, so the signal fills 6 of the 8 divisions.
pub(crate) fn nice_per_div(span: f64) -> f64 {
    let target = (span / 6.0).max(1e-3);
    let decade = 10f64.powf(target.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * decade)
        .find(|&v| v >= target * 0.999)
        .unwrap_or(10.0 * decade)
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
    fn text_over_the_limit_is_cut_at_a_line() {
        let mut big = b"Time (s),CH1 (V)\n".to_vec();
        let line = b"0.000001,1.25\n";
        while big.len() <= MAX_FILE_BYTES + 100 {
            big.extend_from_slice(line);
        }
        let (part, cut) = within_limit(&big).unwrap();
        assert!(cut);
        assert!(part.len() <= MAX_FILE_BYTES);
        assert!(part.ends_with(b"\n"), "a whole line");
        // Binary data over the limit is refused.
        let mut bin = vec![0u8; MAX_FILE_BYTES + 1];
        bin[..4].copy_from_slice(b"AG10");
        assert!(within_limit(&bin).is_err());
        // Small files are not touched.
        assert_eq!(within_limit(b"abc").unwrap(), (&b"abc"[..], false));
    }

    #[test]
    fn unknown_bytes() {
        let e = open(&[0; 10]).unwrap_err();
        assert!(e.0.starts_with("unknown file (10 bytes)"), "{e}");
        assert!(detect(&[0; 10]).is_empty());
    }
}
