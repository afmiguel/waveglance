//! Reader for CSV (and other delimited text) waveform files, as most
//! oscilloscopes and this viewer export them.
//!
//! The reader looks for the block of lines that hold numbers, and the
//! columns that are numbers in most of those lines:
//! - The first such column is the time (s). A Rigol-style file gives a sample
//!   number with `Start` and `Increment` values instead; then
//!   `time = Start + number × Increment`.
//! - The other columns are channels. Their names come from the header line
//!   ("CH1(V)", "1", "ch2_V", …), and "A" / "Amp" in a name or units line
//!   makes a current channel.
//! - Lines before the numbers are the header (Keysight: names, then units;
//!   Tektronix: settings in the first columns of the data lines).
//! - Separators: `,`, `;` or tab. With `;`, a decimal comma is accepted.
//!
//! The files of this viewer (File → Export CSV) also give back the V/div,
//! probe and coupling of each channel, from their `#` lines.

use super::{Confidence, Format, ReadError, nice_per_div};
use crate::model::{
    Acquisition, Capture, Coupling, Display, Samples, Source, Trace, TraceId, Unit,
};

/// CSV and other delimited text.
pub struct CsvFormat;

/// Bytes looked at by [`Format::probe`].
const PROBE_BYTES: usize = 64 * 1024;

impl Format for CsvFormat {
    fn name(&self) -> &'static str {
        "CSV / text table"
    }

    /// Text without NUL bytes, with lines of numbers.
    fn probe(&self, bytes: &[u8]) -> Confidence {
        let head = &bytes[..bytes.len().min(PROBE_BYTES)];
        if head.is_empty() || head.contains(&0) {
            return Confidence::No;
        }
        let text = String::from_utf8_lossy(head);
        let bad = text
            .chars()
            .filter(|&c| c == '\u{FFFD}' || (c.is_control() && !c.is_whitespace()))
            .count();
        if bad * 100 > text.len().max(1) {
            return Confidence::No;
        }
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let delimiter = find_delimiter(&lines);
        let numeric = lines
            .iter()
            .filter(|l| {
                split(l, delimiter)
                    .iter()
                    .filter(|f| number(f, delimiter).is_some())
                    .count()
                    >= 2
            })
            .count();
        if numeric >= 5 || (numeric >= 2 && numeric * 2 >= lines.len()) {
            Confidence::Likely
        } else if numeric >= 1 {
            Confidence::Maybe
        } else {
            Confidence::No
        }
    }

    fn read(&self, bytes: &[u8]) -> Result<Capture, ReadError> {
        read(bytes)
    }
}

/// The column separator: the one of `,` `;` tab that splits the lines into
/// the most fields.
fn find_delimiter(lines: &[&str]) -> char {
    let sample = &lines[lines.len().saturating_sub(200)..];
    // ";" or tab in most lines wins: with ";" the decimal separator can be
    // a comma, so the commas do not count.
    for d in [';', '\t'] {
        if sample.iter().filter(|l| l.contains(d)).count() * 2 > sample.len() {
            return d;
        }
    }
    ','
}

fn split(line: &str, delimiter: char) -> Vec<&str> {
    line.split(delimiter)
        .map(|f| f.trim().trim_matches('"').trim())
        .collect()
}

/// A field as a number; with `;` separators, a decimal comma is accepted.
fn number(field: &str, delimiter: char) -> Option<f64> {
    if field.is_empty() {
        return None;
    }
    let v = if delimiter == ';' && field.contains(',') && !field.contains('.') {
        field.replace(',', ".").parse().ok()
    } else {
        field.parse().ok()
    };
    v.filter(|v: &f64| v.is_finite())
}

/// Channel name from a header field: "CH1(V)" / "ch1_V" / "1" / "C2" → "CH1" / "CH2".
fn channel_label(header: &str, k: usize) -> String {
    let h = header.trim();
    let lower = h.to_lowercase();
    let digit_after = |prefix: &str| {
        lower.find(prefix).and_then(|i| {
            lower[i + prefix.len()..]
                .trim_start()
                .chars()
                .next()
                .filter(char::is_ascii_digit)
        })
    };
    if let Some(d) = digit_after("ch").or_else(|| digit_after("channel")) {
        return format!("CH{d}");
    }
    if h.len() == 1 && h.chars().all(|c| c.is_ascii_digit()) {
        return format!("CH{h}");
    }
    if lower.len() == 2
        && lower.starts_with('c')
        && lower.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
    {
        return format!("CH{}", &h[1..]);
    }
    let unit_word = matches!(
        lower.as_str(),
        "" | "v" | "volt" | "volts" | "a" | "amp" | "amps" | "y" | "value"
    );
    if unit_word {
        format!("CH{}", k + 1)
    } else {
        h.chars().take(12).collect()
    }
}

/// True if a header or units field names amperes.
fn is_current(field: &str) -> bool {
    let f = field.to_lowercase();
    f.contains("(a)")
        || f.ends_with("_a")
        || f == "a"
        || f.starts_with("amp")
        || f.contains("current")
}

/// The maker named in the header lines, if any.
fn vendor(header: &str) -> (String, String) {
    let h = header.to_lowercase();
    // This viewer's own export: "# file: …; source: FNIRSI DPOX180H".
    if let Some(i) = h.find("source:") {
        let source = header[i + 7..].lines().next().unwrap_or("").trim();
        if let Some((v, m)) = source.split_once(' ') {
            return (v.to_owned(), m.to_owned());
        }
        if !source.is_empty() {
            return (source.to_owned(), String::new());
        }
    }
    for (key, name) in [
        ("keysight", "Keysight"),
        ("agilent", "Agilent"),
        ("rigol", "Rigol"),
        ("tektronix", "Tektronix"),
        ("record length", "Tektronix"),
        ("siglent", "Siglent"),
        ("lecroy", "LeCroy"),
        ("rohde", "Rohde & Schwarz"),
        ("owon", "Owon"),
        ("hantek", "Hantek"),
    ] {
        if h.contains(key) {
            return (name.to_owned(), String::new());
        }
    }
    ("CSV".to_owned(), String::new())
}

/// Settings from this viewer's `#` lines: "# CH1: 0.5 V/div, probe 10X, AC".
fn exported_display(header: &str, label: &str) -> Option<(f64, f64, Option<Coupling>)> {
    let line = header.lines().find(|l| {
        l.trim_start_matches('#')
            .trim_start()
            .starts_with(&format!("{label}:"))
    })?;
    let rest = line.split_once(':')?.1;
    let per_div: f64 = rest.split_whitespace().next()?.parse().ok()?;
    let probe = rest
        .split("probe")
        .nth(1)
        .and_then(|p| {
            p.trim()
                .trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.')
                .split('X')
                .next()
        })
        .and_then(|p| p.trim().parse().ok())
        .unwrap_or(1.0);
    let coupling = if rest.contains(", AC") {
        Some(Coupling::Ac)
    } else if rest.contains(", DC") {
        Some(Coupling::Dc)
    } else {
        None
    };
    Some((per_div, probe, coupling))
}

/// Read a CSV file.
///
/// # Errors
/// A [`ReadError`] if no block of number lines with at least two columns is found.
pub fn read(bytes: &[u8]) -> Result<Capture, ReadError> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{FEFF}');
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let delimiter = find_delimiter(&lines);
    let rows: Vec<Vec<&str>> = lines.iter().map(|l| split(l, delimiter)).collect();

    // The data columns: the most common set of number columns (2 or more).
    let signature = |r: &Vec<&str>| -> Vec<usize> {
        r.iter()
            .enumerate()
            .filter(|(_, f)| number(f, delimiter).is_some())
            .map(|(i, _)| i)
            .collect()
    };
    let mut counts: Vec<(Vec<usize>, usize)> = Vec::new();
    for r in &rows {
        let sig = signature(r);
        if sig.len() < 2 {
            continue;
        }
        match counts.iter_mut().find(|(s, _)| *s == sig) {
            Some((_, n)) => *n += 1,
            None => counts.push((sig, 1)),
        }
    }
    // A set of columns also counts the lines where more columns are numbers
    // (Tektronix: settings in the first columns of some data lines).
    let sigs: Vec<Vec<usize>> = counts.iter().map(|(s, _)| s.clone()).collect();
    let covers = |sig: &Vec<usize>| {
        counts
            .iter()
            .filter(|(s, _)| sig.iter().all(|c| s.contains(c)))
            .map(|(_, n)| *n)
            .sum::<usize>()
    };
    // The best coverage; among the sets within 80% of it, the largest
    // (a header line like "x-axis,1,2" must not hide the time column).
    let best = sigs.iter().map(&covers).max().unwrap_or(0);
    let Some(columns) = sigs
        .into_iter()
        .filter(|s| covers(s) * 5 >= best * 4)
        .max_by_key(|s| (s.len(), covers(s)))
    else {
        return Err(ReadError("CSV: no lines with two or more numbers".into()));
    };
    let first_data = rows
        .iter()
        .position(|r| {
            columns
                .iter()
                .all(|&c| r.get(c).is_some_and(|f| number(f, delimiter).is_some()))
        })
        .unwrap_or(0);

    // Header: the text lines before the numbers.
    let header_text = lines[..first_data].join("\n");
    let names: Vec<&str> = rows[..first_data]
        .iter()
        .rev()
        .find(|r| {
            !r.first().is_some_and(|f| f.starts_with('#')) && r.len() > columns[columns.len() - 1]
        })
        .cloned()
        .unwrap_or_default();
    let units_row: Vec<&str> = if first_data >= 2 {
        rows[first_data - 1].clone()
    } else {
        Vec::new()
    };

    // Rigol style: "X,CH1,Start,Increment" with the values in the first data line.
    let lower: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
    let start_col = lower.iter().position(|n| n == "start");
    let inc_col = lower.iter().position(|n| n == "increment");
    let rigol = match (start_col, inc_col) {
        (Some(s), Some(i)) => {
            let first = &rows[first_data];
            first
                .get(s)
                .and_then(|f| number(f, delimiter))
                .zip(first.get(i).and_then(|f| number(f, delimiter)))
        }
        _ => None,
    };
    let value_columns: Vec<usize> = columns
        .iter()
        .copied()
        .skip(1)
        .filter(|c| Some(*c) != start_col && Some(*c) != inc_col)
        .collect();
    let time_col = columns[0];

    let mut warnings = Vec::new();
    let mut times: Vec<f64> = Vec::new();
    let mut values: Vec<Vec<f64>> = vec![Vec::new(); value_columns.len()];
    for r in &rows[first_data..] {
        let Some(t) = r.get(time_col).and_then(|f| number(f, delimiter)) else {
            continue;
        };
        let vals: Vec<Option<f64>> = value_columns
            .iter()
            .map(|&c| r.get(c).and_then(|f| number(f, delimiter)))
            .collect();
        if vals.iter().all(Option::is_none) {
            continue;
        }
        times.push(match rigol {
            Some((start, inc)) => start + t * inc,
            None => t,
        });
        for (k, v) in vals.into_iter().enumerate() {
            values[k].push(v.unwrap_or(f64::NAN));
        }
    }
    let n = times.len();
    if n < 2 {
        return Err(ReadError("CSV: fewer than two samples".into()));
    }
    if times.windows(2).any(|w| w[1] < w[0]) {
        warnings.push("the time column is not in increasing order".into());
    }

    // Evenly spaced? Then store as uniform samples (4 bytes each).
    let dt = (times[n - 1] - times[0]) / (n - 1) as f64;
    let step = (n / 1000).max(1);
    let uniform = dt > 0.0
        && (0..n)
            .step_by(step)
            .all(|i| (times[i] - (times[0] + i as f64 * dt)).abs() <= dt * 1e-3);

    let mut traces = Vec::new();
    for (k, &col) in value_columns.iter().enumerate() {
        let header = names.get(col).copied().unwrap_or("");
        let label = channel_label(header, k);
        let current = is_current(header) || units_row.get(col).is_some_and(|u| is_current(u));
        let v = &values[k];
        let (lo, hi) = v
            .iter()
            .filter(|x| x.is_finite())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| {
                (lo.min(x), hi.max(x))
            });
        let (per_div, probe, coupling) = exported_display(&header_text, &label).unwrap_or((
            nice_per_div(if hi > lo { hi - lo } else { 1.0 }),
            1.0,
            None,
        ));
        let samples = if uniform {
            Samples::Uniform {
                t0: times[0],
                dt,
                values: v.iter().map(|&x| x as f32).collect(),
            }
        } else {
            Samples::Timed(times.iter().zip(v).map(|(&t, &x)| [t, x]).collect())
        };
        let number = label
            .strip_prefix("CH")
            .and_then(|d| d.parse().ok())
            .unwrap_or(k as u8 + 1);
        let mut trace = Trace::new(
            TraceId::Analog(number),
            label,
            samples,
            Display {
                per_div,
                zero_div: 0.0,
                probe,
                coupling,
            },
        );
        if current {
            trace.unit = Unit::Ampere;
        }
        traces.push(trace);
    }

    // The maker can also be in the data lines (Tektronix): look at the start.
    let start_text = lines[..lines.len().min(30)].join("\n");
    let (vendor, model) = vendor(&start_text);
    let time_per_div = header_text
        .lines()
        .find_map(|l| l.trim_start_matches('#').trim().strip_prefix("time/div:"))
        .and_then(|v| v.split_whitespace().next()?.parse().ok());
    Ok(Capture {
        source: Source {
            vendor,
            model,
            format: CsvFormat.name().into(),
        },
        traces,
        time_per_div,
        trigger: None,
        acquisition: Acquisition::RealTime {
            sample_rate_hz: if dt > 0.0 { 1.0 / dt } else { 0.0 },
        },
        record_length: n,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_str(s: &str) -> Capture {
        assert_ne!(CsvFormat.probe(s.as_bytes()), Confidence::No, "probe");
        read(s.as_bytes()).unwrap()
    }

    #[test]
    fn roundtrip_of_the_viewer_export() {
        // A real 1013D capture: export it, read it back.
        let e = &crate::examples::EXAMPLES[0];
        let original = crate::formats::open(e.bytes).unwrap();
        let text = crate::export::csv(&original, e.file, "test");
        let back = read_str(&text);
        assert_eq!(back.traces.len(), original.traces.len());
        assert_eq!(
            (back.source.vendor.as_str(), back.source.model.as_str()),
            ("FNIRSI", "1013D/1014D")
        );
        assert_eq!(back.time_per_div, original.time_per_div);
        for (a, b) in original.traces.iter().zip(&back.traces) {
            assert_eq!(a.label, b.label);
            assert_eq!(a.display.per_div, b.display.per_div);
            assert_eq!(a.display.probe, b.display.probe);
            assert_eq!(a.display.coupling, b.display.coupling);
            assert!(matches!(b.samples, Samples::Uniform { .. }));
            for (p, q) in a.points().iter().zip(b.points()) {
                assert!(
                    (p[0] - q[0]).abs() < 1e-12 && (p[1] - q[1]).abs() < 1e-6,
                    "{p:?} {q:?}"
                );
            }
        }
    }

    #[test]
    fn keysight_style() {
        let w = read_str(
            "x-axis,1,2\nsecond,Volt,Volt\n-1.0e-3,0.1,2.0\n-0.5e-3,0.2,2.1\n0,0.3,2.2\n0.5e-3,0.4,2.3\n1.0e-3,0.5,2.4\n",
        );
        assert_eq!(w.source.vendor, "CSV");
        let labels: Vec<&str> = w.traces.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["CH1", "CH2"]);
        assert_eq!(w.traces[1].points()[2], [0.0, 2.200000047683716]);
        assert_eq!(w.traces[0].sample_interval(), Some(0.5e-3));
    }

    #[test]
    fn rigol_start_increment() {
        let w =
            read_str("X,CH1,Start,Increment,\n0,1.0,-5e-6,1e-6,\n1,2.0,\n2,3.0,\n3,4.0,\n4,5.0,\n");
        let t = &w.traces[0];
        assert_eq!(w.traces.len(), 1);
        assert_eq!(t.label, "CH1");
        let p = t.points();
        assert!(
            (p[0][0] + 5e-6).abs() < 1e-15 && (p[4][0] + 1e-6).abs() < 1e-15,
            "{p:?}"
        );
        assert_eq!(p[4][1], 5.0);
    }

    #[test]
    fn tektronix_columns() {
        let w = read_str(
            "Record Length,5,Points,-2e-6,0.1\nSample Interval,1e-6,s,-1e-6,0.2\nTrigger Point,2,Samples,0,0.3\n,,,1e-6,0.4\n,,,2e-6,0.5\n",
        );
        assert_eq!(w.source.vendor, "Tektronix");
        assert_eq!(w.record_length, 5);
        assert_eq!(w.traces[0].points()[4], [2e-6, 0.5]);
    }

    #[test]
    fn semicolon_and_decimal_comma_and_current() {
        let w = read_str(
            "Time (s);Current (A)\n0,000;0,5\n0,001;0,6\n0,002;0,7\n0,003;0,8\n0,004;0,9\n",
        );
        assert_eq!(w.traces[0].unit, Unit::Ampere);
        assert!((w.traces[0].points()[1][1] - 0.6).abs() < 1e-6);
        assert_eq!(w.traces[0].sample_interval(), Some(0.001));
    }

    #[test]
    fn uneven_times_are_kept() {
        let w = read_str("t,v\n0,1\n1,2\n3,3\n6,4\n10,5\n");
        assert!(matches!(w.traces[0].samples, Samples::Timed(_)));
        assert_eq!(w.traces[0].points()[3], [6.0, 4.0]);
    }

    #[test]
    fn not_csv() {
        assert_eq!(CsvFormat.probe(&[0, 1, 2, 3, 0, 9]), Confidence::No);
        assert_eq!(
            CsvFormat.probe(b"hello world\nno numbers here\n"),
            Confidence::No
        );
        assert!(read(b"just text\n").is_err());
        assert_eq!(nice_per_div(1.07), 0.2);
        assert_eq!(nice_per_div(6.0), 1.0);
    }
}
