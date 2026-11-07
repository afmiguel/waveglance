//! Reader for VCD (Value Change Dump, IEEE 1364) files: the text format of
//! logic analyzers (sigrok/PulseView, Saleae), mixed-signal oscilloscopes and
//! Verilog simulators.
//!
//! ```text
//! $timescale 1 ns $end
//! $scope module top $end
//! $var wire 1 ! clk $end        1-bit signal, identifier "!"
//! $var real 64 " vout $end      analog value
//! $var wire 8 # data $end       bus (not shown yet)
//! $upscope $end
//! $enddefinitions $end
//! #0                            time (in timescale units)
//! 0!                            clk = 0
//! r1.25 "                       vout = 1.25
//! #50
//! 1!
//! ```
//!
//! - 1-bit signals become digital traces (0/1; `x` and `z` are shown as
//!   0.5), drawn as steps.
//! - `real` variables become analog traces (volts).
//! - Buses (more than 1 bit) are not shown yet (a warning says so).

use super::{Confidence, Format, ReadError, nice_per_div};
use crate::model::{Acquisition, Capture, Display, Samples, Source, Trace, TraceId, Unit};

/// VCD.
pub struct VcdFormat;

impl Format for VcdFormat {
    fn name(&self) -> &'static str {
        "VCD (logic analyzer / simulator)"
    }

    fn probe(&self, bytes: &[u8]) -> Confidence {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(64 * 1024)]);
        let starts = head.trim_start().starts_with('$');
        if starts && head.contains("$var") && head.contains("$enddefinitions") {
            Confidence::Sure
        } else if starts && head.contains("$var") {
            Confidence::Likely
        } else {
            Confidence::No
        }
    }

    fn read(&self, bytes: &[u8]) -> Result<Capture, ReadError> {
        read(bytes)
    }
}

/// One variable of the file.
struct Var {
    name: String,
    real: bool,
    /// Changes: (time in timescale units, value).
    changes: Vec<(u64, f64)>,
}

/// Seconds of one timescale unit: "1 ns", "10ps", "100 us".
fn timescale(text: &str) -> Option<f64> {
    let t: String = text.split_whitespace().collect();
    let digits: String = t.chars().take_while(char::is_ascii_digit).collect();
    let n: f64 = digits.parse().ok()?;
    let unit = match &t[digits.len()..] {
        "s" => 1.0,
        "ms" => 1e-3,
        "us" => 1e-6,
        "ns" => 1e-9,
        "ps" => 1e-12,
        "fs" => 1e-15,
        _ => return None,
    };
    Some(n * unit)
}

/// The value of a 1-bit scalar: 0, 1, or 0.5 for x / z / others.
fn bit(c: char) -> f64 {
    match c {
        '0' => 0.0,
        '1' => 1.0,
        _ => 0.5,
    }
}

/// Read a VCD file.
///
/// # Errors
/// A [`ReadError`] if there is no `$var` with values.
pub fn read(bytes: &[u8]) -> Result<Capture, ReadError> {
    let text = String::from_utf8_lossy(bytes);
    let mut tokens = text.split_whitespace().peekable();
    let mut vars: Vec<Var> = Vec::new();
    // identifier → index in `vars` (an identifier can be shared by several
    // $var lines; the first one is used).
    let mut ids: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut skipped_buses: Vec<String> = Vec::new();
    let mut unit_s = None;
    let mut version = String::new();
    let mut warnings = Vec::new();

    // Header: $keyword ... $end blocks until $enddefinitions.
    while let Some(tok) = tokens.next() {
        let mut block = || {
            let mut words = Vec::new();
            for w in tokens.by_ref() {
                if w == "$end" {
                    break;
                }
                words.push(w);
            }
            words
        };
        match tok {
            "$timescale" => unit_s = timescale(&block().join(" ")),
            "$version" => version = block().join(" "),
            "$var" => {
                let w = block();
                if w.len() < 4 {
                    continue;
                }
                let (kind, size, id, name) = (w[0], w[1], w[2], w[3]);
                let real = kind == "real" || kind == "realtime";
                let width: usize = size.parse().unwrap_or(1);
                if !real && width != 1 {
                    skipped_buses.push(format!("{name} ({width} bits)"));
                    continue;
                }
                if !ids.contains_key(id) {
                    ids.insert(id.to_owned(), vars.len());
                    vars.push(Var {
                        name: name.to_owned(),
                        real,
                        changes: Vec::new(),
                    });
                }
            }
            "$enddefinitions" => {
                block();
                break;
            }
            t if t.starts_with('$') => {
                block();
            }
            _ => {}
        }
    }
    if vars.is_empty() {
        return Err(ReadError("VCD: no 1-bit or real variable".into()));
    }
    let unit_s = unit_s.unwrap_or_else(|| {
        warnings.push("no $timescale: 1 ns is used".to_owned());
        1e-9
    });
    if !skipped_buses.is_empty() {
        warnings.push(format!("buses not shown yet: {}", skipped_buses.join(", ")));
    }

    // Value changes.
    let mut now: u64 = 0;
    let mut end: u64 = 0;
    while let Some(tok) = tokens.next() {
        let mut set = |id: &str, v: f64, now: u64| {
            if let Some(&i) = ids.get(id) {
                let ch = &mut vars[i].changes;
                // Keep one value per time: the last one.
                if ch.last().is_some_and(|c| c.0 == now) {
                    ch.pop();
                }
                ch.push((now, v));
            }
        };
        let mut chars = tok.chars();
        match chars.next() {
            Some('#') => {
                now = chars.as_str().parse().unwrap_or(now);
                end = end.max(now);
            }
            Some('$') => {} // $dumpvars, $end, $dumpall…: the values follow as usual
            Some(c @ ('0' | '1' | 'x' | 'X' | 'z' | 'Z')) => set(chars.as_str(), bit(c), now),
            Some('b' | 'B') => {
                // Vector value: only for buses, which are skipped; consume the id.
                tokens.next();
            }
            Some('r' | 'R') => {
                let v: f64 = chars.as_str().parse().unwrap_or(f64::NAN);
                if let Some(id) = tokens.next() {
                    set(id, v, now);
                }
            }
            _ => {}
        }
    }

    let mut traces = Vec::new();
    let mut digital = 0u16;
    for var in vars {
        if var.changes.is_empty() {
            continue;
        }
        let t = |u: u64| u as f64 * unit_s;
        let mut points: Vec<[f64; 2]> = Vec::with_capacity(var.changes.len() * 2 + 1);
        for (k, &(time, v)) in var.changes.iter().enumerate() {
            // Digital: a step (the old value up to the change, then the new one).
            if !var.real && k > 0 {
                points.push([t(time), var.changes[k - 1].1]);
            }
            points.push([t(time), v]);
        }
        // Hold the last value until the end of the record.
        if let Some(&(last_t, last_v)) = var.changes.last()
            && last_t < end
        {
            points.push([t(end), last_v]);
        }
        let trace = if var.real {
            let (lo, hi) = points
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                    (lo.min(p[1]), hi.max(p[1]))
                });
            Trace::new(
                TraceId::Analog(traces.len() as u8 + 1),
                var.name,
                Samples::Timed(points),
                Display {
                    per_div: nice_per_div(if hi > lo { hi - lo } else { 1.0 }),
                    zero_div: 0.0,
                    probe: 1.0,
                    coupling: None,
                },
            )
        } else {
            digital += 1;
            let mut tr = Trace::new(
                TraceId::Digital(digital),
                var.name,
                Samples::Timed(points),
                Display {
                    per_div: 1.0,
                    zero_div: 0.0,
                    probe: 1.0,
                    coupling: None,
                },
            );
            tr.unit = Unit::Logic;
            tr.resolution = Some(1.0);
            tr
        };
        traces.push(trace);
    }
    if traces.is_empty() {
        return Err(ReadError("VCD: no value changes".into()));
    }
    let record_length = traces.iter().map(|t| t.samples.len()).max().unwrap_or(0);
    let span = end as f64 * unit_s;
    let vendor = if version.is_empty() {
        "VCD".to_owned()
    } else {
        version
            .split_whitespace()
            .next()
            .unwrap_or("VCD")
            .to_owned()
    };
    Ok(Capture {
        source: Source {
            vendor,
            model: String::new(),
            format: VcdFormat.name().into(),
        },
        traces,
        time_per_div: (span > 0.0).then_some(span / 10.0),
        trigger: None,
        acquisition: Acquisition::RealTime {
            sample_rate_hz: 1.0 / unit_s,
        },
        record_length,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "$date today $end
$version sigrok 0.5 $end
$timescale 10 ns $end
$scope module logic $end
$var wire 1 ! clk $end
$var wire 1 \" en $end
$var real 64 # vout $end
$var wire 8 $ data $end
$upscope $end
$enddefinitions $end
#0
$dumpvars
0!
x\"
r0.5 #
b00000000 $
$end
#50
1!
1\"
r1.5 #
#100
0!
#150
1!
r2.5 #
#200
0!
";

    #[test]
    fn reads_bits_reals_and_skips_buses() {
        assert_eq!(VcdFormat.probe(SAMPLE.as_bytes()), Confidence::Sure);
        let w = read(SAMPLE.as_bytes()).unwrap();
        assert_eq!(w.source.vendor, "sigrok");
        let labels: Vec<&str> = w.traces.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["clk", "en", "vout"]);
        assert!(w.warnings.iter().any(|m| m.contains("data (8 bits)")));
        let clk = &w.traces[0];
        assert!(clk.is_digital() && clk.unit == Unit::Logic);
        // Steps: at 500 ns the value goes from 0 to 1.
        assert_eq!(&clk.points()[..3], &[[0.0, 0.0], [5e-7, 0.0], [5e-7, 1.0]]);
        // en: unknown (x) shown as 0.5, then 1, held to the end (2 µs).
        let en = &w.traces[1];
        assert_eq!(en.points()[0], [0.0, 0.5]);
        assert_eq!(en.points().last(), Some(&[2e-6, 1.0]));
        let vout = &w.traces[2];
        assert!(!vout.is_digital());
        assert_eq!(vout.points()[1], [5e-7, 1.5]);
        assert_eq!(w.time_per_div, Some(2e-7));
    }

    #[test]
    fn clock_frequency() {
        // clk: rising edges at #50 and #150 of 10 ns: period 1 µs → 1 MHz.
        let w = read(SAMPLE.as_bytes()).unwrap();
        let clk = &w.traces[0];
        let r = crate::measure::measure(clk.points(), &(0.0..=2e-6), 0.1);
        let f = r.get(crate::measure::Measurement::Frequency).unwrap();
        assert!((f - 1e6).abs() < 1.0, "{f}");
    }

    #[test]
    fn not_vcd() {
        assert_eq!(VcdFormat.probe(b"time,v\n0,1\n"), Confidence::No);
        assert!(read(b"$timescale 1ns $end $enddefinitions $end").is_err());
        assert_eq!(timescale("100 ps"), Some(1e-10));
        assert_eq!(timescale("1us"), Some(1e-6));
    }
}
