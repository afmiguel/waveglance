//! Number formatting and small helpers on point lists.

use super::*;

/// Time span of all the channels.
pub fn time_span(channels: &[&[[f64; 2]]]) -> Option<RangeInclusive<f64>> {
    let (lo, hi) = channels
        .iter()
        .flat_map(|c| c.iter())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p[0]), hi.max(p[0]))
        });
    (lo < hi).then_some(lo..=hi)
}

/// Voltage range of the samples with a time inside `time`, plus a 5% margin.
/// `None` if no sample is in that window.
pub fn voltage_span_in(
    channels: &[&[[f64; 2]]],
    time: &RangeInclusive<f64>,
) -> Option<RangeInclusive<f64>> {
    let (lo, hi) = channels
        .iter()
        .flat_map(|c| c.iter())
        .filter(|p| time.contains(&p[0]))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p[1]), hi.max(p[1]))
        });
    if lo > hi {
        return None;
    }
    // A flat signal still gets a visible range.
    let margin = if hi > lo {
        (hi - lo) * 0.05
    } else {
        lo.abs().max(1e-3) * 0.1
    };
    Some(lo - margin..=hi + margin)
}

/// Reduce `points` to about `buckets` min/max pairs, in time order, so that a
/// long record draws fast and its peaks stay visible (like a scope display).
pub fn decimate(points: &[[f64; 2]], buckets: usize) -> Vec<[f64; 2]> {
    if buckets == 0 || points.len() <= buckets * 2 {
        return points.to_vec();
    }
    let size = points.len().div_ceil(buckets);
    points
        .chunks(size)
        .flat_map(|chunk| {
            let (mut lo, mut hi) = (chunk[0], chunk[0]);
            for p in chunk {
                if p[1] < lo[1] {
                    lo = *p;
                }
                if p[1] > hi[1] {
                    hi = *p;
                }
            }
            if lo[0] <= hi[0] { [lo, hi] } else { [hi, lo] }
        })
        .collect()
}

/// The next V/div in the 1-2-5 sequence (1 mV … 500 V): smaller if
/// `finer`, else larger. A value outside the sequence (2.5 V on the 1013D)
/// goes to the nearest step in that direction.
pub fn vdiv_step(v: f64, finer: bool) -> f64 {
    let steps: Vec<f64> = (-3..=2)
        .flat_map(|e| [1.0, 2.0, 5.0].map(|m| m * 10f64.powi(e)))
        .collect();
    let eps = v * 1e-9;
    if finer {
        steps
            .iter()
            .rev()
            .copied()
            .find(|&s| s < v - eps)
            .unwrap_or(steps[0])
    } else {
        steps
            .iter()
            .copied()
            .find(|&s| s > v + eps)
            .unwrap_or(steps[steps.len() - 1])
    }
}

/// Format a value with `digits` significant digits (trailing zeros kept, so
/// the width does not jump), an SI prefix and a unit. With `resolution > 0`,
/// the value is rounded to a multiple of it and no digit below it is shown.
/// Examples: `fmt_sig(1000.0, "Hz", 4, 0.0)` → `1.000 kHz`;
/// `fmt_sig(10e-6, "V", 4, 1e-3)` → `0 V`.
pub fn fmt_sig(value: f64, unit: &str, digits: i32, resolution: f64) -> String {
    if !value.is_finite() {
        return "—".to_owned();
    }
    let mut v = value;
    if resolution > 0.0 {
        v = (v / resolution).round() * resolution;
    }
    if v == 0.0 {
        return format!("0 {unit}");
    }
    // Round to `digits` significant digits first, so that the prefix fits the
    // rounded value (999.96 → 1.000 k, not 1000.0).
    let step = 10f64.powi(v.abs().log10().floor() as i32 - digits + 1);
    v = (v / step).round() * step;
    // The value is already rounded (999.96 → 1000), so the prefix needs no
    // tolerance: 999.7 stays "999.7 Hz", not "0.9997 kHz".
    let (scale, prefix) = si_prefix_exact(v.abs());
    let scaled = v / scale;
    let magnitude = scaled.abs().log10().floor() as i32;
    let mut decimals = (digits - 1 - magnitude).max(0);
    if resolution > 0.0 {
        let res_decimals = (-(resolution / scale).log10() - 1e-9).ceil() as i32;
        decimals = decimals.min(res_decimals.max(0));
    }
    minus(format!("{scaled:.*} {prefix}{unit}", decimals as usize))
}

/// The typographic minus sign "−" (U+2212) in place of the hyphen.
fn minus(text: String) -> String {
    match text.strip_prefix('-') {
        Some(rest) => format!("−{rest}"),
        None => text,
    }
}

/// The SI prefix for a magnitude that is already rounded: (scale, prefix).
fn si_prefix_exact(magnitude: f64) -> (f64, &'static str) {
    let (scale, prefix) = si_prefix(magnitude);
    if magnitude < scale * (1.0 - 1e-12) {
        si_prefix(magnitude * 0.999)
    } else {
        (scale, prefix)
    }
}

/// The SI prefix for a magnitude: (scale, prefix).
pub(super) fn si_prefix(magnitude: f64) -> (f64, &'static str) {
    const PREFIXES: [(f64, &str); 9] = [
        (1e9, "G"),
        (1e6, "M"),
        (1e3, "k"),
        (1.0, ""),
        (1e-3, "m"),
        (1e-6, "µ"),
        (1e-9, "n"),
        (1e-12, "p"),
        (1e-15, "f"),
    ];
    PREFIXES
        .iter()
        .find(|(s, _)| magnitude >= s * 0.9995)
        .copied()
        .unwrap_or((1e-15, "f"))
}

/// Format a value with an SI prefix and unit, for example `0.2, "V"` → `200 mV`.
pub fn si(value: f64, unit: &str) -> String {
    format!("{}{unit}", si_bare(value))
}

/// Format a value with an SI prefix and a trailing space, for example `4e8` → `400 M`.
pub fn si_bare(value: f64) -> String {
    if value == 0.0 || !value.is_finite() {
        return format!("{value} ");
    }
    let (scale, prefix) = si_prefix(value.abs());
    let text = format!("{:.3}", value / scale);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    minus(format!("{text} {prefix}"))
}

/// How bus values are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Radix {
    #[default]
    Hex,
    Dec,
    Bin,
}

impl Radix {
    pub const ALL: [Self; 3] = [Self::Hex, Self::Dec, Self::Bin];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hex => "Hex",
            Self::Dec => "Dec",
            Self::Bin => "Bin",
        }
    }
}

/// A bus value of `bits` bits: "3F" (hex, one digit per 4 bits), "63" or
/// "00111111". "X" if a bit is unknown (NaN).
pub fn bus_text(value: f64, bits: u16, radix: Radix) -> String {
    if !value.is_finite() || value < 0.0 {
        return "X".to_owned();
    }
    // Exact up to 2^53; wider values are rounded by the reader.
    let v = value as u64;
    let bits = usize::from(bits.clamp(1, 64));
    match radix {
        Radix::Hex => format!("{v:0width$X}", width = bits.div_ceil(4)),
        Radix::Dec => v.to_string(),
        Radix::Bin => format!("{v:0bits$b}"),
    }
}
