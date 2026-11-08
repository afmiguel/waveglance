//! Reader for the `.wfm` files of Rigol oscilloscopes:
//!
//! - DS1000Z family (DS1054Z, DS1074Z, DS1104Z, DS1202Z-E, MSO1104Z…):
//!   starts with `01 ff ff ff`.
//! - DS1000E / DS1000D family (DS1052E, DS1102E, DS1102D…): starts with
//!   `a5 a5 00 00`.
//! - DS2000 family (DS2072A, DS2202…): starts with `a5 a5 38 00`.
//!
//! The layouts come from the RigolWFM project by Scott Prahl (BSD 3-Clause,
//! <https://github.com/scottprahl/RigolWFM>), which documents them as Kaitai
//! Struct files, and from Rigol's "Waveform Storage Structure" documents.
//! Samples are 8-bit ADC codes, 25 codes per vertical division.

use super::{Confidence, Format, ReadError};
use crate::model::{
    Acquisition, Capture, Coupling, Display, Samples, Source, Trace, TraceId, Unit,
};

/// Rigol `.wfm`.
pub struct RigolWfmFormat;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Family {
    Ds1000z,
    Ds1000e,
    Ds2000,
}

fn family(bytes: &[u8]) -> Option<Family> {
    match bytes.get(..4)? {
        [0x01, 0xff, 0xff, 0xff] => Some(Family::Ds1000z),
        [0xa5, 0xa5, 0x00, 0x00] => Some(Family::Ds1000e),
        [0xa5, 0xa5, 0x38, 0x00] => Some(Family::Ds2000),
        _ => None,
    }
}

impl Format for RigolWfmFormat {
    fn name(&self) -> &'static str {
        "Rigol (.wfm)"
    }

    /// The magic number is short, so fixed header fields are checked too:
    /// the DS1000Z header size (0xd8) and the zero bytes of the DS1000E
    /// header.
    fn probe(&self, bytes: &[u8]) -> Confidence {
        let Some(f) = family(bytes) else {
            return Confidence::No;
        };
        let fixed = match f {
            Family::Ds1000z => u16_at(bytes, 84) == Some(0xd8),
            Family::Ds1000e => bytes
                .get(6..20)
                .is_some_and(|z| z[..10].iter().chain(&z[11..]).all(|&c| c == 0)),
            Family::Ds2000 => bytes.get(44..46) == Some(&[1, 0][..]),
        };
        if fixed {
            Confidence::Sure
        } else {
            Confidence::No
        }
    }

    fn read(&self, bytes: &[u8]) -> Result<Capture, ReadError> {
        read(bytes)
    }
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}
fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn i32_at(b: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}
fn i64_at(b: &[u8], at: usize) -> Option<i64> {
    Some(i64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}
fn f32_at(b: &[u8], at: usize) -> Option<f64> {
    Some(f64::from(f32::from_le_bytes(
        b.get(at..at + 4)?.try_into().ok()?,
    )))
}
fn text_at(b: &[u8], at: usize, n: usize) -> String {
    let Some(s) = b.get(at..at + n) else {
        return String::new();
    };
    let end = s.iter().position(|&c| c == 0).unwrap_or(n);
    String::from_utf8_lossy(&s[..end]).trim().to_owned()
}

fn short() -> ReadError {
    ReadError("Rigol .wfm: the file ends early".into())
}

/// Probe factors of the DS1000Z and DS2000 (an index in the file).
fn probe_factor(code: u8) -> f64 {
    const F: [f64; 16] = [
        0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0,
        1000.0,
    ];
    F.get(usize::from(code)).copied().unwrap_or(1.0)
}

fn coupling(code: u8) -> Option<Coupling> {
    match code {
        0 => Some(Coupling::Dc),
        1 => Some(Coupling::Ac),
        _ => None,
    }
}

/// `x` rounded to 6 significant digits: the V/div values are stored as
/// f32 or in µV, so 0.05 comes back as 0.0500000007.
fn tidy(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let p = 10f64.powi(5 - x.abs().log10().floor() as i32);
    (x * p).round() / p
}

/// One decoded channel before it becomes a [`Trace`].
struct Channel {
    number: u8,
    /// Volts per division (with the probe), always positive.
    per_div: f64,
    /// Volts = `gain · (code − mid) + offset`.
    gain: f64,
    mid: f64,
    offset: f64,
    /// Position of 0 V on the scope screen, in divisions from the centre.
    zero_div: f64,
    probe: f64,
    coupling: Option<Coupling>,
    unit: Unit,
    codes: Vec<u8>,
    /// Time of the first sample and time between samples.
    t0: f64,
    dt: f64,
}

impl Channel {
    fn into_trace(self) -> Trace {
        let values = self
            .codes
            .iter()
            .map(|&c| (self.gain * (f64::from(c) - self.mid) + self.offset) as f32)
            .collect();
        let mut t = Trace::new(
            TraceId::Analog(self.number),
            format!("CH{}", self.number),
            Samples::Uniform {
                t0: self.t0,
                dt: self.dt,
                values,
            },
            Display {
                per_div: tidy(self.per_div),
                zero_div: self.zero_div,
                probe: self.probe,
                coupling: self.coupling,
            },
        );
        t.unit = self.unit;
        t.resolution = Some(self.gain.abs());
        // Codes 0 and 255 are the limits of the ADC.
        let a = self.gain * (0.0 - self.mid) + self.offset;
        let b = self.gain * (255.0 - self.mid) + self.offset;
        t.range = Some((a.min(b), a.max(b)));
        t
    }
}

/// What every family gives back.
struct Parsed {
    model: String,
    time_per_div: Option<f64>,
    channels: Vec<Channel>,
    warnings: Vec<String>,
}

/// Read a Rigol `.wfm` file.
///
/// # Errors
/// A [`ReadError`] if the file is not a known Rigol `.wfm` or has no channel.
pub fn read(bytes: &[u8]) -> Result<Capture, ReadError> {
    let parsed = match family(bytes) {
        Some(Family::Ds1000z) => ds1000z(bytes)?,
        Some(Family::Ds1000e) => ds1000e(bytes)?,
        Some(Family::Ds2000) => ds2000(bytes)?,
        None => return Err(ReadError("not a Rigol .wfm file".into())),
    };
    if parsed.channels.is_empty() {
        return Err(ReadError(
            "Rigol .wfm: no analog channel is on in this file (logic analyzer data is not read)"
                .into(),
        ));
    }
    let dt = parsed.channels[0].dt;
    let record_length = parsed
        .channels
        .iter()
        .map(|c| c.codes.len())
        .max()
        .unwrap_or(0);
    let warnings = parsed.warnings;
    Ok(Capture {
        source: Source {
            vendor: "Rigol".into(),
            model: parsed.model,
            format: RigolWfmFormat.name().into(),
        },
        traces: parsed
            .channels
            .into_iter()
            .map(Channel::into_trace)
            .collect(),
        time_per_div: parsed.time_per_div,
        trigger: None,
        acquisition: Acquisition::RealTime {
            sample_rate_hz: if dt > 0.0 { 1.0 / dt } else { 0.0 },
        },
        record_length,
        warnings,
    })
}

/// DS1000Z: a 64-byte file header, a waveform header at 64, then the samples
/// after the "horizontal" block. With several channels the samples are
/// interleaved: 2 channels → 2 bytes per point, 3 or 4 → 4 bytes (CH4 CH3
/// CH2 CH1).
fn ds1000z(b: &[u8]) -> Result<Parsed, ReadError> {
    let model = text_at(b, 8, 20);
    let firmware = text_at(b, 28, 20);
    let ps_per_div = u64_at(b, 64).ok_or_else(short)?;
    let ps_offset = i64_at(b, 72).ok_or_else(short)?;
    let mask = *b.get(88).ok_or_else(short)?;
    let enabled: Vec<bool> = (0..4).map(|i| mask & (1 << i) != 0).collect();
    let memory_depth = u32_at(b, 116).ok_or_else(short)? as usize;
    let rate = f32_at(b, 120).ok_or_else(short)? * 1e9;
    let horizontal_size = u32_at(b, 256).ok_or_else(short)? as usize;
    let horizontal_offset = u32_at(b, 260).ok_or_else(short)? as usize;
    if rate <= 0.0 {
        return Err(ReadError("Rigol .wfm: no sample rate".into()));
    }
    let dt = 1.0 / rate;
    let count = enabled.iter().filter(|&&e| e).count().max(1);
    let stride = if count == 3 { 4 } else { count };
    let points = memory_depth / stride;
    let start = horizontal_offset + horizontal_size;
    let data = b
        .get(start..)
        .ok_or_else(short)?
        .get(..points * stride)
        .unwrap_or_else(|| &b[start.min(b.len())..]);
    let mut warnings = Vec::new();
    if data.len() < points * stride {
        warnings.push(format!(
            "the file holds {} of {} points",
            data.len() / stride,
            points
        ));
    }
    let time_offset = ps_offset as f64 * 1e-12;
    let n = data.len() / stride;
    let t0 = time_offset - n as f64 * dt / 2.0;
    // Older firmware with two channels uses another vertical bias.
    let sp3_two = firmware == "00.04.04.SP3" && count == 2;
    let mut channels = Vec::new();
    for (i, on) in enabled.iter().enumerate() {
        if !on {
            continue;
        }
        let head = 124 + 28 * i;
        let coupling_code = *b.get(head + 1).ok_or_else(short)?;
        let probe = probe_factor(*b.get(head + 4).ok_or_else(short)?);
        let scale = f32_at(b, head + 8).ok_or_else(short)?;
        let shift = f32_at(b, head + 12).ok_or_else(short)?;
        let inverted = *b.get(head + 16).ok_or_else(short)? != 0;
        let unit = match b.get(head + 17) {
            Some(1) => Unit::Ampere,
            _ => Unit::Volt,
        };
        let vpd = if inverted { -scale } else { scale };
        let bias = if sp3_two {
            if shift < 0.0 { vpd / 5.0 } else { 0.0 }
        } else {
            vpd
        };
        // Byte of this channel in each point: 2 channels → the first one on
        // the odd bytes; 4 bytes → CH4 CH3 CH2 CH1.
        let lane = match stride {
            2 => usize::from(!enabled[..i].iter().any(|&e| e)),
            4 => 3 - i,
            _ => 0,
        };
        let codes: Vec<u8> = data.iter().skip(lane).step_by(stride).copied().collect();
        channels.push(Channel {
            number: i as u8 + 1,
            per_div: scale.abs(),
            gain: vpd / 20.0,
            mid: 127.0,
            offset: bias - shift,
            zero_div: if scale != 0.0 { shift / scale } else { 0.0 },
            probe,
            coupling: coupling(coupling_code),
            unit,
            codes,
            t0,
            dt,
        });
    }
    Ok(Parsed {
        model,
        time_per_div: (ps_per_div > 0).then_some(ps_per_div as f64 * 1e-12),
        channels,
        warnings,
    })
}

/// DS1000E / DS1000D: a 276-byte header, then CH1 and CH2 samples one after
/// the other. Higher codes are lower voltages; 0 V is code 125.
fn ds1000e(b: &[u8]) -> Result<Parsed, ReadError> {
    let roll_stop = u32_at(b, 20).ok_or_else(short)? as usize;
    let ch1_depth = u32_at(b, 28).ok_or_else(short)? as usize;
    let ch2_depth_raw = u32_at(b, 232).ok_or_else(short)? as usize;
    // Time headers: scale_measured and offset_measured in picoseconds.
    let time = 84;
    let rate = f32_at(b, time + 16).ok_or_else(short)?;
    let scale_ps = i64_at(b, time + 20).ok_or_else(short)?;
    let offset_ps = i64_at(b, time + 28).ok_or_else(short)?;
    let trigger_mode = *b.get(142).ok_or_else(short)?;
    let time2 = 236;
    // Alternate trigger: CH2 has its own time base.
    let offset2_ps = if trigger_mode == 4 {
        i64_at(b, time2 + 28).ok_or_else(short)?
    } else {
        offset_ps
    };
    if rate <= 0.0 {
        return Err(ReadError("Rigol .wfm: no sample rate".into()));
    }
    let dt = 1.0 / rate;
    let skip = if roll_stop == 0 { 0 } else { roll_stop + 2 };
    let mut warnings = Vec::new();
    if skip > 0 {
        warnings.push("roll mode: the record stops at the roll position".into());
    }
    let mut at = 276;
    let mut channels = Vec::new();
    for i in 0..2 {
        let head = 34 + 24 * i;
        let on = *b.get(head + 15).ok_or_else(short)? != 0;
        if !on {
            continue;
        }
        let depth = if i == 1 && ch2_depth_raw == 0 {
            ch1_depth
        } else if i == 1 {
            ch2_depth_raw
        } else {
            ch1_depth
        };
        let points = depth.saturating_sub(skip);
        let probe = f32_at(b, head + 10).ok_or_else(short)?;
        let probe = if probe > 0.0 { probe } else { 1.0 };
        let scale_uv = f64::from(i32_at(b, head + 18).ok_or_else(short)?);
        let shift = f64::from(i16::from_le_bytes(
            b.get(head + 22..head + 24)
                .ok_or_else(short)?
                .try_into()
                .unwrap_or_default(),
        ));
        let Some(codes) = b.get(at..at + points) else {
            warnings.push(format!("CH{}: the file ends early", i + 1));
            break;
        };
        at += points + skip;
        let vpd = 1e-6 * scale_uv * probe;
        let gain = vpd / 25.0;
        let offset = if i == 0 { offset_ps } else { offset2_ps };
        let t_off = offset as f64 * 1e-12;
        channels.push(Channel {
            number: i as u8 + 1,
            per_div: vpd.abs(),
            // Inverted codes: volts = gain · (125 − code) − shift · gain.
            gain: -gain,
            mid: 125.0,
            offset: -shift * gain,
            zero_div: shift / 25.0,
            probe,
            coupling: None,
            unit: Unit::Volt,
            codes: codes.to_vec(),
            t0: t_off - depth as f64 * dt / 2.0,
            dt,
        });
    }
    Ok(Parsed {
        model: "DS1000E/D".into(),
        time_per_div: (scale_ps > 0).then_some(scale_ps as f64 * 1e-12),
        channels,
        warnings,
    })
}

/// DS2000: a header with the offset of each channel's samples; higher codes
/// are higher voltages; 0 V is code 127 before the offset.
fn ds2000(b: &[u8]) -> Result<Parsed, ReadError> {
    // The "model" field holds the serial number (DS2A…, DS2D…).
    let serial = text_at(b, 4, 20);
    let firmware = text_at(b, 24, 20);
    let mask = *b.get(64).ok_or_else(short)?;
    let interwoven = b.get(65).ok_or_else(short)? & 1 != 0;
    let rate = f32_at(b, 96).ok_or_else(short)?;
    let scale_ps = u64_at(b, 104).ok_or_else(short)?;
    let mut offset_ps = i64_at(b, 112).ok_or_else(short)?;
    let storage_depth = u32_at(b, 244).ok_or_else(short)? as usize;
    let z_pt_offset = u32_at(b, 248).ok_or_else(short)? as usize;
    let wfm_len = u32_at(b, 252).ok_or_else(short)? as usize;
    if rate <= 0.0 {
        return Err(ReadError("Rigol .wfm: no sample rate".into()));
    }
    // Old DS2A firmware: the stored offset is not the trigger position.
    if serial.starts_with("DS2A") && firmware == "00.03.00.01.03" {
        offset_ps = 0;
    }
    let dt = 1.0 / rate;
    let time_offset = offset_ps as f64 * 1e-12 + z_pt_offset as f64 * dt;
    let raw_depth = if interwoven { wfm_len / 2 } else { wfm_len };
    let raw = |k: usize| -> Option<&[u8]> {
        let off = u32_at(b, 68 + 4 * k)? as usize;
        if off == 0 {
            return None;
        }
        b.get(off + z_pt_offset..off + z_pt_offset + raw_depth)
    };
    let mut warnings = Vec::new();
    let mut channels = Vec::new();
    for i in 0..4 {
        let has_data = u32_at(b, 68 + 4 * i).is_some_and(|o| o > 0);
        if mask & (1 << i) == 0 || !has_data {
            continue;
        }
        let codes: Vec<u8> = if interwoven {
            // The memory of CH1 and CH2 together hold one channel at twice
            // the rate.
            let (Some(a), Some(c)) = (raw(0), raw(1)) else {
                warnings.push(format!("CH{}: the file ends early", i + 1));
                continue;
            };
            a.iter().zip(c).flat_map(|(&x, &y)| [x, y]).collect()
        } else {
            let Some(r) = raw(i) else {
                warnings.push(format!("CH{}: the file ends early", i + 1));
                continue;
            };
            r.to_vec()
        };
        let head = 120 + 28 * i;
        let enabled_temp = *b.get(head).ok_or_else(short)?;
        let coupling_code = b.get(head + 1).ok_or_else(short)? >> 6;
        let ratio = *b.get(head + 4).ok_or_else(short)?;
        let impedance = *b.get(head + 7).ok_or_else(short)?;
        let vpd = f32_at(b, head + 8).ok_or_else(short)?;
        let offset = f32_at(b, head + 12).ok_or_else(short)?;
        let (t16, t17) = (b[head + 16], b[head + 17]);
        let legacy = enabled_temp == 1;
        let (inv, unit) = if legacy { (t16, t17) } else { (t17, t16) };
        // Newer firmware stores a 1X probe as 0.01X with 50 Ω (code 0).
        let ratio = if !legacy && ratio == 0 && impedance == 0 {
            6
        } else {
            ratio
        };
        let signed = if inv == 1 { -vpd } else { vpd };
        channels.push(Channel {
            number: i as u8 + 1,
            per_div: vpd.abs(),
            gain: signed / 25.0,
            mid: 127.0,
            offset: -offset,
            zero_div: if vpd != 0.0 { offset / vpd } else { 0.0 },
            probe: probe_factor(ratio),
            coupling: coupling(coupling_code),
            unit: if unit == 1 { Unit::Ampere } else { Unit::Volt },
            t0: time_offset - storage_depth.max(codes.len()) as f64 * dt / 2.0,
            codes,
            dt,
        });
    }
    Ok(Parsed {
        model: "DS2000".into(),
        time_per_div: (scale_ps > 0).then_some(scale_ps as f64 * 1e-12),
        channels,
        warnings,
    })
}
