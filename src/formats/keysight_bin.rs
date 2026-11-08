//! Reader for the binary waveform files (`.bin`) of Keysight and Agilent
//! oscilloscopes (InfiniiVision 1000/2000/3000/4000 X, 6000 and others), and
//! of Rigol oscilloscopes (MSO5000, DHO800/DHO1000), which use the same
//! layout with "RG" in place of "AG".
//!
//! Rigol "03" (DHO800/1000) has 64-bit sizes, like Keysight "03". Rigol
//! stores the x origin as the pre-trigger time, positive (the first sample
//! is at −x origin).
//!
//! The format is documented by Keysight (user manuals, "Binary Data (.bin)
//! Format"). Little-endian:
//!
//! ```text
//! File header      "AG" + version ("01", "03", "10"), file size (u32; u64 for 03),
//!                  number of waveforms (u32)
//! For each waveform:
//!   Waveform header  header size, type, buffers, points, count, x display
//!                    range (f32), x display origin, x increment, x origin
//!                    (f64), x units, y units, date, time, frame (model:serial),
//!                    label, time tag, segment index
//!   For each buffer:
//!     Data header    header size, buffer type (u16), bytes per point (u16),
//!                    buffer size (u32; u64 for 03)
//!     Data           float32 values (types 1 normal, 2 maximum, 3 minimum),
//!                    or bytes (5 logic, 6 digital), or i32 counts (4)
//! ```
//!
//! The file does not store the V/div; a V/div that fits the signal is used.
//! The time/div is the x display range / 10.

use super::{Confidence, Format, ReadError, nice_per_div};
use crate::model::{Acquisition, Capture, Display, Samples, Source, Trace, TraceId, Unit};

/// Keysight / Agilent / Rigol `.bin`.
pub struct KeysightBinFormat;

impl Format for KeysightBinFormat {
    fn name(&self) -> &'static str {
        "Keysight / Agilent / Rigol (.bin)"
    }

    /// "AG" or "RG" and a known version; sure if the size field is the file size.
    fn probe(&self, bytes: &[u8]) -> Confidence {
        let Some(version) = version(bytes) else {
            return Confidence::No;
        };
        let size = if version == 3 {
            bytes
                .get(4..12)
                .map(|b| u64::from_le_bytes(b.try_into().unwrap_or_default()))
        } else {
            bytes
                .get(4..8)
                .map(|b| u64::from(u32::from_le_bytes(b.try_into().unwrap_or_default())))
        };
        if size == Some(bytes.len() as u64) {
            Confidence::Sure
        } else {
            Confidence::Likely
        }
    }

    fn read(&self, bytes: &[u8]) -> Result<Capture, ReadError> {
        read(bytes)
    }
}

/// The format version (1, 3 or 10) if the file starts with "AG" or "RG"
/// and a known version.
fn version(bytes: &[u8]) -> Option<u32> {
    if !matches!(bytes.get(..2)?, b"AG" | b"RG") {
        return None;
    }
    let v: u32 = std::str::from_utf8(bytes.get(2..4)?).ok()?.parse().ok()?;
    matches!(v, 1 | 3 | 10).then_some(v)
}

/// A little-endian reader over the file bytes.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], ReadError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or_else(|| {
                ReadError(format!(
                    "Keysight .bin: the file ends early (needs {n} bytes at {})",
                    self.at
                ))
            })?;
        let b = &self.bytes[self.at..end];
        self.at = end;
        Ok(b)
    }
    fn u16(&mut self) -> Result<u16, ReadError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().unwrap_or_default(),
        ))
    }
    fn u32(&mut self) -> Result<u32, ReadError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().unwrap_or_default(),
        ))
    }
    fn u64(&mut self) -> Result<u64, ReadError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().unwrap_or_default(),
        ))
    }
    fn f32(&mut self) -> Result<f32, ReadError> {
        Ok(f32::from_le_bytes(
            self.take(4)?.try_into().unwrap_or_default(),
        ))
    }
    fn f64(&mut self) -> Result<f64, ReadError> {
        Ok(f64::from_le_bytes(
            self.take(8)?.try_into().unwrap_or_default(),
        ))
    }
    fn text(&mut self, n: usize) -> Result<String, ReadError> {
        let b = self.take(n)?;
        let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        Ok(String::from_utf8_lossy(&b[..end]).trim().to_owned())
    }
}

/// Read a Keysight / Agilent `.bin` file.
///
/// # Errors
/// A [`ReadError`] if the file is not a valid `.bin` file or has no analog waveform.
pub fn read(bytes: &[u8]) -> Result<Capture, ReadError> {
    let version =
        version(bytes).ok_or_else(|| ReadError("not a Keysight or Rigol .bin file".into()))?;
    let wide = version == 3;
    let rigol = bytes.starts_with(b"RG");
    let mut r = Reader { bytes, at: 4 };
    if wide {
        r.u64()?;
    } else {
        r.u32()?;
    }
    let n_waveforms = r.u32()?;
    let mut traces = Vec::new();
    let mut warnings = Vec::new();
    let mut model = String::new();
    let mut time_per_div = None;
    let mut screen = None;
    let mut record_length = 0;
    for _ in 0..n_waveforms {
        let start = r.at;
        let header_size = r.u32()? as usize;
        let waveform_type = r.u32()?;
        let n_buffers = r.u32()?;
        let points = r.u32()? as usize;
        let _count = r.u32()?;
        let x_display_range = r.f32()?;
        let x_display_origin = r.f64()?;
        let x_increment = r.f64()?;
        let x_origin = r.f64()?;
        let _x_units = r.u32()?;
        let y_units = r.u32()?;
        let _date = r.text(16)?;
        let _time = r.text(16)?;
        let frame = r.text(24)?;
        let label = r.text(16)?;
        // Skip the rest of the header (time tag, segment, extensions).
        r.at = start + header_size;

        if model.is_empty() {
            model = frame.split(':').next().unwrap_or("").trim().to_owned();
        }
        if time_per_div.is_none() && x_display_range > 0.0 {
            time_per_div = Some(f64::from(x_display_range) / 10.0);
            // Rigol stores the display origin as a positive number, like
            // the x origin.
            let start = if rigol {
                -x_display_origin.abs()
            } else {
                x_display_origin
            };
            screen = Some((start, start + f64::from(x_display_range)));
        }
        let mut values: Option<Vec<f32>> = None;
        for _ in 0..n_buffers {
            let data_header = r.u32()? as usize;
            let buffer_type = r.u16()?;
            let bytes_per_point = r.u16()?;
            let size = if wide {
                r.u64()? as usize
            } else {
                r.u32()? as usize
            };
            // Skip extra data-header bytes, if any.
            let used = 8 + if wide { 8 } else { 4 };
            if data_header > used {
                r.take(data_header - used)?;
            }
            let data = r.take(size)?;
            match buffer_type {
                // Normal or maximum (peak detect): float32 values.
                1 | 2 if bytes_per_point == 4 && values.is_none() => {
                    values = Some(
                        data.chunks_exact(4)
                            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                            .collect(),
                    );
                    if buffer_type == 2 {
                        warnings.push(format!(
                            "{label}: peak detect; the maximum values are shown"
                        ));
                    }
                }
                1..=3 => {}
                _ => warnings.push(format!(
                    "{label}: {} waveform not shown (only analog waveforms are read)",
                    match buffer_type {
                        4 => "histogram",
                        5 | 6 => "digital",
                        _ => "unknown",
                    }
                )),
            }
        }
        let Some(values) = values else { continue };
        // Some files of old MSO5000 firmware hold values like 1e33: not
        // samples. Refuse them instead of drawing nonsense.
        if values.iter().any(|v| !v.is_finite() || v.abs() > 1e6) {
            warnings.push(format!(
                "{label}: values out of range (damaged file?); not shown"
            ));
            continue;
        }
        if waveform_type == 4 || waveform_type == 5 {
            continue;
        }
        if values.len() != points {
            warnings.push(format!(
                "{label}: {points} points in the header, {} in the data",
                values.len()
            ));
        }
        record_length = record_length.max(values.len());
        let (lo, hi) = values
            .iter()
            .filter(|v| v.is_finite())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &v| {
                (lo.min(v), hi.max(v))
            });
        let span = if hi > lo { f64::from(hi - lo) } else { 1.0 };
        // Analog channels are labelled "1".."4" (Keysight) or "CH1".."CH4";
        // others keep their name. No label (Rigol MSO5000): by position.
        let next = traces.len() as u8 + 1;
        let number = label.strip_prefix("CH").unwrap_or(&label).parse::<u8>();
        let (id, name) = match number {
            Ok(n) => (TraceId::Analog(n), format!("CH{n}")),
            Err(_) if label.is_empty() => (TraceId::Analog(next), format!("CH{next}")),
            Err(_) => (TraceId::Analog(next), label.clone()),
        };
        let mut trace = Trace::new(
            id,
            name,
            Samples::Uniform {
                // Rigol: the x origin is the pre-trigger time, positive.
                t0: if rigol { -x_origin.abs() } else { x_origin },
                dt: x_increment,
                values,
            },
            Display {
                per_div: nice_per_div(span),
                zero_div: 0.0,
                probe: 1.0,
                coupling: None,
            },
        );
        if y_units == 4 {
            trace.unit = Unit::Ampere;
        }
        traces.push(trace);
    }
    if traces.is_empty() {
        return Err(ReadError(if warnings.is_empty() {
            "Keysight .bin: no analog waveform in the file".into()
        } else {
            format!(
                "Keysight .bin: no analog waveform shown ({})",
                warnings.join("; ")
            )
        }));
    }
    let dt = traces[0].sample_interval().unwrap_or(0.0);
    Ok(Capture {
        source: Source {
            vendor: if rigol { "Rigol" } else { "Keysight" }.into(),
            model,
            format: KeysightBinFormat.name().into(),
        },
        traces,
        time_per_div,
        screen,
        trigger: None,
        acquisition: Acquisition::RealTime {
            sample_rate_hz: if dt > 0.0 { 1.0 / dt } else { 0.0 },
        },
        record_length,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic AG10 file: one waveform "1", a ramp of `n` float32 values.
    fn fake(n: usize, label: &str, y_units: u32) -> Vec<u8> {
        let mut w = Vec::new();
        w.extend(140u32.to_le_bytes()); // header size
        w.extend(1u32.to_le_bytes()); // normal
        w.extend(1u32.to_le_bytes()); // buffers
        w.extend((n as u32).to_le_bytes());
        w.extend(1u32.to_le_bytes()); // count
        w.extend(1e-3f32.to_le_bytes()); // x display range
        w.extend(0f64.to_le_bytes());
        w.extend(1e-6f64.to_le_bytes()); // x increment
        w.extend((-5e-4f64).to_le_bytes()); // x origin
        w.extend(2u32.to_le_bytes()); // s
        w.extend(y_units.to_le_bytes());
        w.extend([0u8; 32]); // date, time
        let mut frame = b"DSO-X 1102G:CN1".to_vec();
        frame.resize(24, 0);
        w.extend(frame);
        let mut l = label.as_bytes().to_vec();
        l.resize(16, 0);
        w.extend(l);
        w.extend(0f64.to_le_bytes());
        w.extend(0u32.to_le_bytes());
        assert_eq!(w.len(), 140);
        w.extend(12u32.to_le_bytes());
        w.extend(1u16.to_le_bytes());
        w.extend(4u16.to_le_bytes());
        w.extend(((n * 4) as u32).to_le_bytes());
        for i in 0..n {
            w.extend((i as f32 * 0.5).to_le_bytes());
        }
        let mut f = b"AG10".to_vec();
        f.extend(((12 + w.len()) as u32).to_le_bytes());
        f.extend(1u32.to_le_bytes());
        f.extend(w);
        f
    }

    #[test]
    fn reads_a_synthetic_file() {
        let b = fake(10, "1", 1);
        assert_eq!(KeysightBinFormat.probe(&b), Confidence::Sure);
        let w = read(&b).unwrap();
        assert_eq!(w.source.model, "DSO-X 1102G");
        assert_eq!(w.time_per_div, Some(f64::from(1e-3f32) / 10.0));
        let t = &w.traces[0];
        assert_eq!(t.label, "CH1");
        assert_eq!(t.unit, Unit::Volt);
        assert_eq!(t.points()[2], [-5e-4 + 2e-6, 1.0]);
        let w = read(&fake(4, "3", 4)).unwrap();
        assert_eq!(
            (w.traces[0].label.as_str(), w.traces[0].unit),
            ("CH3", Unit::Ampere)
        );
    }

    #[test]
    fn rejects_bad_files() {
        assert_eq!(KeysightBinFormat.probe(b"AGxx"), Confidence::No);
        assert_eq!(KeysightBinFormat.probe(b"RIFF...."), Confidence::No);
        let mut b = fake(10, "1", 1);
        b.truncate(100);
        assert_eq!(
            KeysightBinFormat.probe(&b),
            Confidence::Likely,
            "size does not match"
        );
        assert!(read(&b).is_err());
    }
}
