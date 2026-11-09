//! Reader for the text files of Digilent WaveForms (Analog Discovery 1, 2
//! and 3, Electronics Explorer, ADP…): File → Export of the Oscilloscope
//! and the Logic Analyzer, as `.csv` (comma) or `.txt` (tab).
//!
//! The file starts with `#` lines:
//!
//! ```text
//! #Digilent WaveForms Oscilloscope Acquisition
//! #Device Name: Discovery3
//! #Serial Number: SN:210415B9FDDD
//! #Date Time: 2025-03-05 15:48:44.807.213.070
//! #Sample rate: 1e+08Hz
//! #Samples: 8192
//! #Trigger: Source: Channel 1 Type: Edge Condition: Rising Level: 0 V …
//! #Channel 1: Range: 500 mV/div Offset: 0 V Attenuation: 1 X
//! ```
//!
//! then a header line (`Time (s),Channel 1 (V),Channel 2 (V)`, or for the
//! Logic Analyzer `Time (s),DIO 0,Bus,SPI…`) and one line per sample. The
//! time is in seconds, 0 at the trigger.
//!
//! Columns:
//! - analog (`Channel 1 (V)`, math, references): volts (or amperes);
//! - digital (only `0`, `1` and `X`): a logic line, ready for the decoders;
//!   `X` (no value) becomes 0.5, like an unknown VCD value;
//! - bus and interpreter values of the Logic Analyzer (`hFFF`, `3501`):
//!   a bus trace (`X` = no value).
//!
//! The values of each column are kept as `f32` (analog) or as the times of
//! their changes (digital), and the time column is not kept when it is
//! even: a long Record-mode file (millions of lines) stays small in memory.

use super::{Confidence, Format, ReadError, nice_per_div};
use crate::model::{
    Acquisition, Capture, Coupling, Display, Samples, Source, Trace, TraceId, Trigger, Unit,
};

/// Digilent WaveForms text export.
pub struct DigilentFormat;

/// The first line of every WaveForms export.
const SIGNATURE: &str = "#Digilent WaveForms";

impl Format for DigilentFormat {
    fn name(&self) -> &'static str {
        "Digilent WaveForms (.csv, .txt)"
    }

    fn probe(&self, bytes: &[u8]) -> Confidence {
        let head = &bytes[..bytes.len().min(256)];
        let head = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
        if head.starts_with(SIGNATURE.as_bytes()) {
            Confidence::Sure
        } else {
            Confidence::No
        }
    }

    fn read(&self, bytes: &[u8]) -> Result<Capture, ReadError> {
        read(bytes)
    }
}

/// The settings in the `#` lines.
#[derive(Debug, Default)]
struct Header {
    /// "Oscilloscope Acquisition", "Logic Analyzer Acquisition", …
    kind: String,
    device: String,
    sample_rate: Option<f64>,
    trigger: Option<String>,
    /// `#Channel 1: Range: …` lines, by channel number.
    channels: Vec<(u8, String)>,
}

impl Header {
    fn logic(&self) -> bool {
        self.kind.starts_with("Logic")
    }
}

fn parse_header(lines: &[&str]) -> Header {
    let mut h = Header::default();
    for line in lines {
        let line = line.trim_start_matches('\u{FEFF}').trim();
        let Some(body) = line.strip_prefix('#') else {
            continue;
        };
        if let Some(kind) = body.strip_prefix("Digilent WaveForms") {
            h.kind = kind.trim().to_owned();
            continue;
        }
        let Some((key, value)) = body.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Device Name" => h.device = value.to_owned(),
            "Sample rate" => {
                h.sample_rate = number(value.trim_end_matches("Hz").trim()).filter(|r| *r > 0.0);
            }
            "Trigger" => h.trigger = Some(value.to_owned()),
            k => {
                if let Some(n) = k
                    .strip_prefix("Channel ")
                    .and_then(|n| n.trim().parse().ok())
                {
                    h.channels.push((n, value.to_owned()));
                }
            }
        }
    }
    h
}

/// A plain number (`1e+08`, `-0.000102385`).
fn number(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// A value with an SI unit: `500 mV`, `4 V`, `200 uV`, `1.5 kV` → volts.
fn si_value(s: &str) -> Option<f64> {
    let s = s.trim();
    let end = s
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')))
        .unwrap_or(s.len());
    let v: f64 = s[..end].trim().parse().ok()?;
    let unit = s[end..].trim();
    let scale = match unit.chars().next() {
        Some('p') => 1e-12,
        Some('n') => 1e-9,
        Some('u' | 'µ' | 'μ') => 1e-6,
        Some('m') => 1e-3,
        Some('k') => 1e3,
        Some('M') => 1e6,
        _ => 1.0,
    };
    Some(v * scale)
}

/// The text after `key` up to the next known key, in a settings line like
/// `Range: 500 mV/div Offset: 0 V Attenuation: 1 X`.
fn field<'a>(settings: &'a str, key: &str) -> Option<&'a str> {
    const KEYS: [&str; 12] = [
        "Range:",
        "Offset:",
        "Attenuation:",
        "Coupling:",
        "Sample mode:",
        "Source:",
        "Type:",
        "Condition:",
        "Level:",
        "Hyst.:",
        "HoldOff:",
        "Hysteresis:",
    ];
    let start = settings.find(key)? + key.len();
    let rest = &settings[start..];
    let end = KEYS
        .iter()
        .filter_map(|k| rest.find(k))
        .min()
        .unwrap_or(rest.len());
    Some(rest[..end].trim())
}

/// The trigger from `Source: Channel 1 Type: Edge Condition: Rising …`.
fn trigger(settings: &str) -> Option<Trigger> {
    let source = field(settings, "Source:")?;
    let source = match source.strip_prefix("Channel ") {
        Some(n) => format!("CH{}", n.trim()),
        None => source.to_owned(),
    };
    let condition = field(settings, "Condition:").unwrap_or("");
    let kind = field(settings, "Type:").filter(|t| !t.is_empty());
    Some(Trigger {
        mode: kind.unwrap_or("Trigger").to_owned(),
        rising: !condition.starts_with("Falling"),
        source: Some(source),
    })
}

/// The column separator of the header line: tab, `;` or `,`.
fn delimiter(header: &str) -> char {
    ['\t', ';', ',']
        .into_iter()
        .find(|&d| header.contains(d))
        .unwrap_or(',')
}

/// What a column holds, found from its values.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    /// Volts or amperes.
    Analog,
    /// Only 0, 1 and X.
    Digital,
    /// A number per sample (bus or interpreter value); `bits` from the
    /// hexadecimal width or the largest value.
    Bus,
}

/// A value of a bus or interpreter column: `hFFF`, `0x1F`, `b0101` or a
/// decimal number. Returns the value and the number of bits its text gives
/// (hexadecimal and binary digits), if any.
fn bus_value(s: &str) -> Option<(f64, Option<u16>)> {
    let s = s.trim();
    let (digits, radix, bits_per_digit) = if let Some(h) = s.strip_prefix("0x") {
        (h, 16, 4)
    } else if let Some(h) = s.strip_prefix('h') {
        (h, 16, 4)
    } else if let Some(b) = s.strip_prefix('b') {
        (b, 2, 1)
    } else {
        return s
            .parse::<u64>()
            .ok()
            .map(|v| (v as f64, None))
            .or_else(|| number(s).map(|v| (v, None)));
    };
    let v = u64::from_str_radix(digits, radix).ok()?;
    let bits = u16::try_from(digits.len() * bits_per_digit).ok();
    Some((v as f64, bits))
}

/// "No value" in a data line.
fn missing(s: &str) -> bool {
    let s = s.trim();
    s.is_empty() || s.eq_ignore_ascii_case("x")
}

/// One column while it is read.
struct Column {
    name: String,
    kind: Kind,
    /// Analog values (one per line).
    values: Vec<f32>,
    /// Digital and bus: (line, value) at each change.
    changes: Vec<(usize, f64)>,
    /// Bus: bits from the text (hexadecimal width).
    bits: u16,
    max: f64,
}

/// Read a WaveForms export.
///
/// # Errors
/// A [`ReadError`] if the file has no header line, no time column or no
/// data lines.
pub fn read(bytes: &[u8]) -> Result<Capture, ReadError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ReadError("Digilent WaveForms: the file is not UTF-8 text".into()))?;
    let mut lines = text.lines().enumerate();

    // The # lines, then the column names.
    let mut comments = Vec::new();
    let mut names_line = None;
    for (_, line) in lines.by_ref() {
        let l = line.trim_start_matches('\u{FEFF}');
        if l.starts_with('#') {
            comments.push(l);
        } else if !l.trim().is_empty() {
            names_line = Some(l);
            break;
        }
    }
    let header = parse_header(&comments);
    let names_line =
        names_line.ok_or_else(|| ReadError("Digilent WaveForms: no data in the file".into()))?;
    let delim = delimiter(names_line);
    let names: Vec<&str> = names_line.split(delim).map(str::trim).collect();
    if names.len() < 2 || !names[0].starts_with("Time") {
        return Err(ReadError(format!(
            "Digilent WaveForms: {} files without a time column are not read \
             (first column: \"{}\")",
            if header.kind.is_empty() {
                "these"
            } else {
                &header.kind
            },
            names[0]
        )));
    }
    let decimal_comma = delim == ';';
    let parse = |s: &str| -> Option<f64> {
        if decimal_comma {
            number(&s.replace(',', "."))
        } else {
            number(s)
        }
    };

    // The data lines. The kind of each column is decided from the first
    // data lines and corrected when a later value does not fit.
    let data: Vec<&str> = lines
        .map(|(_, l)| l)
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .collect();
    if data.is_empty() {
        return Err(ReadError("Digilent WaveForms: no data lines".into()));
    }
    let n_cols = names.len() - 1;
    let mut kinds = vec![Kind::Digital; n_cols];
    let mut seen = vec![false; n_cols];
    for line in &data {
        for (c, f) in line.split(delim).skip(1).take(n_cols).enumerate() {
            if missing(f) {
                continue;
            }
            seen[c] = true;
            let f = f.trim();
            kinds[c] = match (kinds[c], f) {
                (Kind::Digital, "0" | "1") => Kind::Digital,
                (Kind::Analog, _) => Kind::Analog,
                (_, f)
                    if bus_value(f).is_some_and(|(v, bits)| bits.is_some() || v.fract() == 0.0) =>
                {
                    Kind::Bus
                }
                _ => Kind::Analog,
            };
        }
    }
    let mut warnings = Vec::new();
    let mut cols: Vec<Column> = names[1..]
        .iter()
        .zip(&kinds)
        .map(|(name, &kind)| {
            // An oscilloscope channel in volts is analog even if it only
            // holds 0 and 1; a bus is only a Logic Analyzer thing.
            let volts = name.ends_with("(V)") || name.ends_with("(A)");
            let kind = match kind {
                Kind::Digital | Kind::Bus if volts => Kind::Analog,
                Kind::Bus if !header.logic() => Kind::Analog,
                k => k,
            };
            Column {
                name: (*name).to_owned(),
                kind,
                values: Vec::new(),
                changes: Vec::new(),
                bits: 0,
                max: 0.0,
            }
        })
        .collect();
    for (c, col) in cols.iter().enumerate() {
        if !seen[c] {
            warnings.push(format!("{}: no values; not shown", col.name));
        }
    }
    for col in &mut cols {
        if col.kind == Kind::Analog {
            col.values.reserve(data.len());
        }
    }

    // Times: kept only if they are not even.
    let mut t0 = None;
    let mut dt = header.sample_rate.map(|r| 1.0 / r);
    let mut times: Option<Vec<f64>> = None;
    let mut bad_lines = 0usize;
    let mut rows = 0usize;
    for line in &data {
        let mut fields = line.split(delim);
        let Some(t) = fields.next().and_then(parse) else {
            bad_lines += 1;
            continue;
        };
        let i = rows;
        rows += 1;
        let start = *t0.get_or_insert(t);
        if i == 1 && dt.is_none() {
            dt = Some(t - start).filter(|d| *d > 0.0);
        }
        match (&mut times, dt) {
            (Some(ts), _) => ts.push(t),
            (None, Some(d)) if (t - (start + i as f64 * d)).abs() <= d * 1e-2 => {}
            (None, _) => {
                // Not even: keep every time from here on.
                let mut ts: Vec<f64> = (0..i)
                    .map(|k| start + k as f64 * dt.unwrap_or(0.0))
                    .collect();
                ts.reserve(data.len() - i);
                ts.push(t);
                times = Some(ts);
            }
        }
        for (col, f) in cols.iter_mut().zip(fields.chain(std::iter::repeat(""))) {
            match col.kind {
                Kind::Analog => col.values.push(parse(f).map_or(f32::NAN, |v| v as f32)),
                Kind::Digital | Kind::Bus => {
                    let v = if missing(f) {
                        if col.kind == Kind::Digital {
                            0.5
                        } else {
                            f64::NAN
                        }
                    } else if col.kind == Kind::Digital {
                        if f.trim() == "1" { 1.0 } else { 0.0 }
                    } else {
                        match bus_value(f) {
                            Some((v, bits)) => {
                                col.bits = col.bits.max(bits.unwrap_or(0));
                                col.max = col.max.max(v);
                                v
                            }
                            None => f64::NAN,
                        }
                    };
                    let same = col
                        .changes
                        .last()
                        .is_some_and(|&(_, last)| last == v || (last.is_nan() && v.is_nan()));
                    if !same {
                        col.changes.push((i, v));
                    }
                }
            }
        }
    }
    if rows < 2 {
        return Err(ReadError(
            "Digilent WaveForms: fewer than two data lines".into(),
        ));
    }
    if bad_lines > 0 {
        warnings.push(format!("{bad_lines} lines without a time were skipped"));
    }
    let start = t0.unwrap_or(0.0);
    let step = dt.unwrap_or(1.0);
    let time_of = |i: usize| match &times {
        Some(ts) => ts[i],
        None => start + i as f64 * step,
    };
    let end = time_of(rows - 1);

    let mut traces = Vec::new();
    let mut digital = 0u16;
    let mut analog = 0u8;
    for col in cols {
        let channel_no = col
            .name
            .strip_prefix("Channel ")
            .and_then(|r| r.split_whitespace().next())
            .and_then(|n| n.parse::<u8>().ok());
        match col.kind {
            Kind::Analog => {
                if col.values.iter().all(|v| v.is_nan()) {
                    continue;
                }
                analog = analog.saturating_add(1);
                let channel = channel_no.unwrap_or(analog);
                let label = match channel_no {
                    Some(n) => format!("CH{n}"),
                    None => col
                        .name
                        .trim_end_matches("(V)")
                        .trim_end_matches("(A)")
                        .trim()
                        .to_owned(),
                };
                let unit = if col.name.ends_with("(A)") {
                    Unit::Ampere
                } else {
                    Unit::Volt
                };
                let settings = channel_no.and_then(|n| {
                    header
                        .channels
                        .iter()
                        .find(|(k, _)| *k == n)
                        .map(|(_, s)| s.as_str())
                });
                let (lo, hi) = col
                    .values
                    .iter()
                    .filter(|v| v.is_finite())
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &v| {
                        (lo.min(v), hi.max(v))
                    });
                let per_div = settings
                    .and_then(|s| field(s, "Range:"))
                    .and_then(|r| si_value(r.trim_end_matches("/div")))
                    .filter(|v| *v > 0.0)
                    .unwrap_or_else(|| {
                        nice_per_div(if hi > lo { f64::from(hi - lo) } else { 1.0 })
                    });
                let offset = settings
                    .and_then(|s| field(s, "Offset:"))
                    .and_then(si_value)
                    .unwrap_or(0.0);
                let probe = settings
                    .and_then(|s| field(s, "Attenuation:"))
                    .and_then(|a| number(a.trim_end_matches(['X', 'x']).trim()))
                    .filter(|a| *a > 0.0)
                    .unwrap_or(1.0);
                let coupling = settings
                    .and_then(|s| field(s, "Coupling:"))
                    .and_then(|c| match c {
                        "AC" => Some(Coupling::Ac),
                        "DC" => Some(Coupling::Dc),
                        _ => None,
                    });
                let display = Display {
                    per_div,
                    // WaveForms adds the offset to the signal on the screen.
                    zero_div: offset / per_div,
                    probe,
                    coupling,
                };
                let samples = match &times {
                    None => Samples::Uniform {
                        t0: start,
                        dt: step,
                        values: col.values,
                    },
                    Some(ts) => Samples::Timed(
                        ts.iter()
                            .zip(&col.values)
                            .map(|(&t, &v)| [t, f64::from(v)])
                            .collect(),
                    ),
                };
                let mut trace = Trace::new(TraceId::Analog(channel), label, samples, display);
                trace.unit = unit;
                traces.push(trace);
            }
            Kind::Digital | Kind::Bus => {
                digital = digital.saturating_add(1);
                // Steps: the old value up to each change, then the new one;
                // the last value is held to the end of the record.
                let mut points = Vec::with_capacity(col.changes.len() * 2 + 1);
                for (k, &(i, v)) in col.changes.iter().enumerate() {
                    let t = time_of(i);
                    if k > 0 {
                        points.push([t, col.changes[k - 1].1]);
                    }
                    points.push([t, v]);
                }
                if let Some(&(i, v)) = col.changes.last()
                    && time_of(i) < end
                {
                    points.push([end, v]);
                }
                let mut trace = Trace::new(
                    TraceId::Digital(digital),
                    col.name,
                    Samples::Timed(points),
                    Display {
                        per_div: 1.0,
                        zero_div: 0.0,
                        probe: 1.0,
                        coupling: None,
                    },
                );
                trace.unit = Unit::Logic;
                trace.resolution = Some(1.0);
                trace.time_resolution = Some(step);
                if col.kind == Kind::Bus {
                    let from_max = (col.max.max(1.0) + 1.0).log2().ceil() as u16;
                    trace.bus = Some(col.bits.max(from_max).max(2));
                }
                traces.push(trace);
            }
        }
    }
    if traces.is_empty() {
        return Err(ReadError(
            "Digilent WaveForms: no columns with values".into(),
        ));
    }
    let sample_rate = header.sample_rate.unwrap_or(1.0 / step);
    let span = end - start;
    let model = header.device.clone();
    Ok(Capture {
        source: Source {
            vendor: "Digilent".into(),
            model,
            format: DigilentFormat.name().into(),
        },
        traces,
        // WaveForms shows 10 divisions.
        time_per_div: (span > 0.0).then_some(span / 10.0),
        screen: None,
        trigger: header.trigger.as_deref().and_then(trigger),
        acquisition: Acquisition::RealTime {
            sample_rate_hz: sample_rate,
        },
        record_length: rows,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCOPE: &str = "\u{FEFF}#Digilent WaveForms Oscilloscope Acquisition
#Device Name: Discovery3
#Serial Number: SN:210415B9FDDD
#Date Time: 2025-03-05 15:48:44.807.213.070
#Sample rate: 1e+06Hz
#Samples: 4
#Trigger: Source: Channel 1 Type: Edge Condition: Falling Level: 0 V Hyst.: Auto HoldOff: 0 s
#Channel 1: Range: 500 mV/div Offset: -1 V Attenuation: 10 X
#Channel 2: Range: 2 V/div Offset: 0 V Attenuation: 1 X

Time (s),Channel 1 (V),Channel 2 (V)
-2e-06,0.5,1
-1e-06,0.25,1
0,0,0
1e-06,-0.25,0
";

    #[test]
    fn oscilloscope_file() {
        assert_eq!(DigilentFormat.probe(SCOPE.as_bytes()), Confidence::Sure);
        let c = read(SCOPE.as_bytes()).unwrap();
        assert_eq!(c.source.title(), "Digilent Discovery3");
        assert_eq!(c.traces.len(), 2);
        let ch1 = c.trace("CH1").unwrap();
        assert!(!ch1.is_digital(), "volts stay analog, also 0/1 values");
        assert!((ch1.display.per_div - 0.5).abs() < 1e-12);
        assert!((ch1.display.zero_div + 2.0).abs() < 1e-12);
        assert_eq!(ch1.display.probe, 10.0);
        match &ch1.samples {
            Samples::Uniform { t0, dt, values } => {
                assert_eq!(*t0, -2e-6);
                assert!((dt - 1e-6).abs() < 1e-18);
                assert_eq!(values, &vec![0.5, 0.25, 0.0, -0.25]);
            }
            s => panic!("{s:?}"),
        }
        assert!(!c.trace("CH2").unwrap().is_digital());
        let t = c.trigger.unwrap();
        assert_eq!(
            (t.mode.as_str(), t.rising, t.source.as_deref()),
            ("Edge", false, Some("CH1"))
        );
        assert_eq!(c.record_length, 4);
        assert!(c.warnings.is_empty(), "{:?}", c.warnings);
    }

    #[test]
    fn logic_file_with_tabs_bus_and_unknown_values() {
        let file = "#Digilent WaveForms Logic Analyzer Acquisition\n#Device Name: Discovery2\n\
                    #Sample rate: 1e+06Hz\n#Samples: 5\n\n\
                    Time (s)\tDIO 0\tSCK\tBus\tSPI\n\
                    0\t0\t1\th0F\tX\n\
                    1e-06\t1\t1\th0F\tX\n\
                    2e-06\t1\t0\thFF\t300\n\
                    3e-06\t0\t0\thFF\t300\n\
                    4e-06\t0\t1\th0A\t7\n";
        let c = read(file.as_bytes()).unwrap();
        assert_eq!(c.traces.len(), 4);
        let dio0 = c.trace("DIO 0").unwrap();
        assert!(dio0.is_digital() && !dio0.is_bus());
        assert_eq!(
            dio0.points(),
            &[
                [0.0, 0.0],
                [1e-6, 0.0],
                [1e-6, 1.0],
                [3e-6, 1.0],
                [3e-6, 0.0],
                [4e-6, 0.0]
            ]
        );
        let bus = c.trace("Bus").unwrap();
        assert_eq!(bus.bus, Some(8), "two hexadecimal digits");
        let spi = c.trace("SPI").unwrap();
        assert_eq!(spi.bus, Some(9), "300 needs 9 bits");
        assert!(spi.points()[0][1].is_nan(), "X: no value");
        assert_eq!(c.source.model, "Discovery2");
        assert!(c.trigger.is_none());
    }

    #[test]
    fn uneven_times_are_kept() {
        let file = "#Digilent WaveForms Oscilloscope Acquisition\n\
                    Time (s),Channel 1 (V)\n0,1\n1,2\n2,3\n10,4\n11,5\n";
        let c = read(file.as_bytes()).unwrap();
        let ch1 = c.trace("CH1").unwrap();
        assert!(matches!(ch1.samples, Samples::Timed(_)));
        assert_eq!(ch1.points()[3], [10.0, 4.0]);
        assert_eq!(ch1.points()[1], [1.0, 2.0]);
    }

    #[test]
    fn semicolons_with_decimal_commas() {
        let file = "#Digilent WaveForms Oscilloscope Acquisition\n#Sample rate: 2Hz\n\
                    Time (s);Channel 1 (V)\n0;1,5\n0,5;-2,25\n1;0\n";
        let c = read(file.as_bytes()).unwrap();
        assert_eq!(c.trace("CH1").unwrap().points()[1], [0.5, -2.25]);
    }

    #[test]
    fn other_instruments_are_refused_with_a_reason() {
        let file =
            "#Digilent WaveForms Spectrum Analyzer\nFrequency (Hz),Trace 1 (dBV)\n1,2\n2,3\n";
        let e = read(file.as_bytes()).unwrap_err();
        assert!(e.0.contains("Spectrum Analyzer"), "{e}");
    }

    #[test]
    fn trigger_without_spaces() {
        let t = trigger("Source: ManualCondition: Rising").unwrap();
        assert_eq!(t.source.as_deref(), Some("Manual"));
        assert!(t.rising);
    }
}
