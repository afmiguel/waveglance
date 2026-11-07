//! Reader for the binary `.wav` files of the FNIRSI DPOX180H.
//!
//! The file is NOT a RIFF/WAV audio file. It is a big-endian dump of the
//! scope state. All offsets below are absolute file offsets.
//!
//! ```text
//! 0x0000  Section table   7 x u32 BE (settings, screen, waveform: start + size; total size)
//! 0x0032  Settings        variable length (a table near 0x2B8 changes its size)
//! ....    Screen buffer   u16 width, u16 height, width*height RGB565 thumbnail
//! ....    Waveform data   u16 BE ADC samples: CH1 (if on), then CH2 (if on)
//! ```
//!
//! Fields used here (all in the fixed part of the settings, before the
//! variable-length table, so their offsets do not move):
//!
//! | Offset | Type | Field                                      |
//! |--------|------|--------------------------------------------|
//! | 0x66   | u8   | CH1 enabled (0 = off)                      |
//! | 0x6E   | u64  | CH1 volts/div in microvolts                |
//! | 0x7E   | u16  | CH1 ground marker, screen pixels (100 = centre) |
//! | 0x82   | u64  | CH1 probe factor x 10000 (10000 = 1X, 100000 = 10X) |
//! | 0xA6   | u8   | CH2 enabled                                |
//! | 0xAE   | u64  | CH2 volts/div in microvolts                |
//! | 0xBE   | u16  | CH2 ground marker                          |
//! | 0xC2   | u64  | CH2 probe factor x 10000                   |
//! | 0x12D  | u32  | samples per channel                        |
//! | 0x13D  | u32  | sample rate in Hz (overflows above 4.29 GS/s) |
//! | 0x1D9  | u32  | time/div in picoseconds                    |
//!
//! Sources: the firmware analysis in github.com/Rav263/fnirsi-wave-decoder
//! (section table, enable flags, sample count, time/div), plus checks made
//! against the screen thumbnails stored in real files (V/div in microvolts,
//! probe factor, ground marker, ETS frame layout, trigger position).
//!
//! The V/div value is the value that the scope shows, so it already includes
//! the probe factor: voltages are at the probe tip. A 1X capture at 200 mV/div
//! and a 10X capture at 2 V/div of the same signal have the same ADC codes.

use super::{AdcScale, Confidence, Format, ReadError, adc_trace};
use crate::model::{Acquisition, Capture, Source, Trace};
use binrw::{BinRead, BinReaderExt};
use std::io::Cursor;

/// ADC code for the vertical centre of the screen.
pub const ADC_CENTER: f64 = 6400.0;
/// ADC codes per vertical division (8 divisions = 0..12800).
pub const ADC_COUNTS_PER_DIV: f64 = 1600.0;
/// Highest ADC code. Samples at 0 or this value are clipped.
pub const ADC_MAX: u16 = 12800;
/// Horizontal divisions on the scope screen.
pub const SCREEN_DIVS_X: f64 = 12.0;
/// Screen pixels per division (the screen is 300 x 200 pixels).
pub const PIXELS_PER_DIV: f64 = 25.0;
/// Vertical pixel position of the screen centre (ground marker units).
pub const MARKER_CENTER_PX: f64 = 100.0;
/// Samples in one equivalent-time (ETS) frame: one sample per screen pixel.
pub const ETS_FRAME_LEN: usize = (SCREEN_DIVS_X * PIXELS_PER_DIV) as usize;

/// Errors that the parser can return.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// The file is too short to contain the header.
    TooShort(usize),
    /// The header could not be decoded.
    Header(String),
    /// The section table does not agree with the file.
    BadSectionTable(String),
    /// Both channels are off, so the file contains no samples.
    NoChannels,
    /// The time/div or sample rate field is zero.
    BadTiming(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort(n) => write!(f, "file is too short ({n} bytes) for a DPOX180H waveform"),
            Self::Header(e) => write!(f, "cannot read the header: {e}"),
            Self::BadSectionTable(e) => write!(f, "not a DPOX180H waveform file: {e}"),
            Self::NoChannels => {
                write!(f, "the file contains no channel data (CH1 and CH2 are off)")
            }
            Self::BadTiming(e) => write!(f, "invalid timing data: {e}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Raw header fields, read with `binrw` from their absolute offsets.
#[derive(BinRead, Debug, Clone, PartialEq)]
#[br(big)]
pub struct RawHeader {
    pub settings_start: u32,
    pub settings_size: u32,
    pub screen_start: u32,
    pub screen_size: u32,
    pub wave_start: u32,
    pub wave_size: u32,
    pub total_size: u32,

    #[br(seek_before = std::io::SeekFrom::Start(0x66))]
    pub ch1_enabled: u8,
    #[br(seek_before = std::io::SeekFrom::Start(0x6E))]
    pub ch1_uv_per_div: u64,
    #[br(seek_before = std::io::SeekFrom::Start(0x7E))]
    pub ch1_marker_px: u16,
    #[br(seek_before = std::io::SeekFrom::Start(0x82))]
    pub ch1_probe_x10000: u64,

    #[br(seek_before = std::io::SeekFrom::Start(0xA6))]
    pub ch2_enabled: u8,
    #[br(seek_before = std::io::SeekFrom::Start(0xAE))]
    pub ch2_uv_per_div: u64,
    #[br(seek_before = std::io::SeekFrom::Start(0xBE))]
    pub ch2_marker_px: u16,
    #[br(seek_before = std::io::SeekFrom::Start(0xC2))]
    pub ch2_probe_x10000: u64,

    #[br(seek_before = std::io::SeekFrom::Start(0x12D))]
    pub sample_count: u32,
    #[br(seek_before = std::io::SeekFrom::Start(0x13D))]
    pub sample_rate_hz: u32,
    #[br(seek_before = std::io::SeekFrom::Start(0x1D9))]
    pub time_per_div_ps: u32,
}

/// The smallest file that can hold all header fields.
pub const MIN_HEADER_LEN: usize = 0x1D9 + 4;

/// The DPOX180H `.wav` format.
pub struct DpoxFormat;

impl Format for DpoxFormat {
    fn name(&self) -> &'static str {
        "FNIRSI DPOX180H (.wav)"
    }

    fn probe(&self, bytes: &[u8]) -> Confidence {
        if looks_like_dpox180h(bytes) {
            Confidence::Sure
        } else {
            Confidence::No
        }
    }

    fn read(&self, bytes: &[u8]) -> Result<Capture, ReadError> {
        read(bytes).map_err(|e| ReadError(e.to_string()))
    }
}

/// A DPOX180H file starts with its section table: settings at offset 50 and
/// a total size equal to the file size.
fn looks_like_dpox180h(bytes: &[u8]) -> bool {
    let be32 = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    be32(0) == Some(0x32) && be32(0x18) == Some(bytes.len() as u32)
}

/// Read the bytes of a DPOX180H `.wav` file.
///
/// # Errors
/// Returns a [`ParseError`] if the bytes are not a valid DPOX180H waveform.
pub fn read(bytes: &[u8]) -> Result<Capture, ParseError> {
    if bytes.len() < MIN_HEADER_LEN {
        return Err(ParseError::TooShort(bytes.len()));
    }
    let header: RawHeader = Cursor::new(bytes)
        .read_be()
        .map_err(|e| ParseError::Header(e.to_string()))?;
    check_section_table(&header, bytes.len())?;

    let mut warnings = Vec::new();
    let ch1_on = header.ch1_enabled != 0;
    let ch2_on = header.ch2_enabled != 0;
    let active = usize::from(ch1_on) + usize::from(ch2_on);
    if active == 0 || header.wave_size == 0 {
        return Err(ParseError::NoChannels);
    }

    // The waveform section size is the reliable sample count.
    let wave_size = header.wave_size as usize;
    if !wave_size.is_multiple_of(active * 2) {
        return Err(ParseError::BadSectionTable(format!(
            "waveform size {wave_size} is not a multiple of {active} channel(s) x 2 bytes"
        )));
    }
    let n = wave_size / (active * 2);
    if header.sample_count as usize != n {
        warnings.push(format!(
            "header says {} samples per channel, waveform section holds {n}",
            header.sample_count
        ));
    }

    if header.time_per_div_ps == 0 {
        return Err(ParseError::BadTiming("time/div is zero".into()));
    }
    let time_per_div_s = f64::from(header.time_per_div_ps) * 1e-12;
    let acquisition = detect_acquisition(&header, n, time_per_div_s)?;
    if let Acquisition::Equivalent { frames } = acquisition
        && !n.is_multiple_of(ETS_FRAME_LEN)
    {
        warnings.push(format!(
            "equivalent-time buffer of {n} samples is not a whole number of {ETS_FRAME_LEN}-point frames ({frames} used)"
        ));
    }

    let start = header.wave_start as usize;
    let samples = |index: usize| -> Vec<u16> {
        let from = start + index * n * 2;
        bytes[from..from + n * 2]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect()
    };

    let mut slot = 0;
    let mut make = |on: bool, uv: u64, marker: u16, probe: u64, number: u8| -> Option<Trace> {
        if !on {
            return None;
        }
        let name = format!("CH{number}");
        let raw = samples(slot);
        slot += 1;
        if uv == 0 {
            warnings.push(format!("{name} V/div is zero; 1 V/div is used"));
        }
        let volts_per_div = if uv == 0 { 1.0 } else { uv as f64 * 1e-6 };
        if probe == 0 {
            warnings.push(format!("{name} probe factor is zero; 1X is used"));
        }
        let probe_factor = if probe == 0 {
            1.0
        } else {
            probe as f64 / 10_000.0
        };
        Some(build_trace(
            number,
            &raw,
            volts_per_div,
            marker,
            probe_factor,
            acquisition,
            time_per_div_s,
        ))
    };
    let ch1 = make(
        ch1_on,
        header.ch1_uv_per_div,
        header.ch1_marker_px,
        header.ch1_probe_x10000,
        1,
    );
    let ch2 = make(
        ch2_on,
        header.ch2_uv_per_div,
        header.ch2_marker_px,
        header.ch2_probe_x10000,
        2,
    );

    Ok(Capture {
        source: Source {
            vendor: "FNIRSI".into(),
            model: "DPOX180H".into(),
            format: DpoxFormat.name().into(),
        },
        traces: [ch1, ch2].into_iter().flatten().collect(),
        time_per_div: Some(time_per_div_s),
        trigger: None,
        acquisition,
        record_length: n,
        warnings,
    })
}

fn check_section_table(h: &RawHeader, len: usize) -> Result<(), ParseError> {
    let bad = |msg: String| Err(ParseError::BadSectionTable(msg));
    let (s, ss) = (u64::from(h.settings_start), u64::from(h.settings_size));
    let (b, bs) = (u64::from(h.screen_start), u64::from(h.screen_size));
    let (w, ws) = (u64::from(h.wave_start), u64::from(h.wave_size));
    let total = u64::from(h.total_size);
    if h.settings_start != 0x32 {
        return bad(format!("settings start is {s}, expected 50"));
    }
    if s + ss != b || b + bs != w || w + ws != total {
        return bad("the sections are not contiguous".into());
    }
    if total != len as u64 {
        return bad(format!(
            "header gives file size {total}, actual size is {len}"
        ));
    }
    Ok(())
}

/// Decide between real-time and equivalent-time sampling.
///
/// In real time, `sample_rate * screen_time` is the number of samples on the
/// screen, and it is at least one per pixel. At ETS timebases the stored
/// rate (5 GS/s) overflows the u32 field, so this value is too small.
fn detect_acquisition(h: &RawHeader, n: usize, tdiv: f64) -> Result<Acquisition, ParseError> {
    let rate = f64::from(h.sample_rate_hz);
    let on_screen = rate * tdiv * SCREEN_DIVS_X;
    if rate > 0.0 && on_screen >= ETS_FRAME_LEN as f64 {
        return Ok(Acquisition::RealTime {
            sample_rate_hz: rate,
        });
    }
    if n < ETS_FRAME_LEN {
        return Err(ParseError::BadTiming(format!(
            "{n} samples per channel is less than one {ETS_FRAME_LEN}-point screen frame"
        )));
    }
    Ok(Acquisition::Equivalent {
        frames: n / ETS_FRAME_LEN,
    })
}

/// One channel: the codes of one acquisition (one frame in ETS mode) to
/// volts, with the trigger (t = 0) at the centre.
fn build_trace(
    number: u8,
    raw: &[u16],
    volts_per_div: f64,
    marker_px: u16,
    probe: f64,
    acq: Acquisition,
    tdiv: f64,
) -> Trace {
    let zero_code = ADC_CENTER
        + (f64::from(marker_px) - MARKER_CENTER_PX) * (ADC_COUNTS_PER_DIV / PIXELS_PER_DIV);
    let scale = AdcScale {
        per_div: volts_per_div,
        counts_per_div: ADC_COUNTS_PER_DIV,
        zero_code,
        center_code: ADC_CENTER,
        code_max: f64::from(ADC_MAX),
    };
    // Time of sample i: t = (i - trigger_index) * dt. Trigger = t = 0.
    let (len, dt) = match acq {
        Acquisition::RealTime { sample_rate_hz } => (raw.len(), 1.0 / sample_rate_hz),
        // One ETS frame is one screen: 12 divisions in 300 points.
        Acquisition::Equivalent { .. } => {
            (ETS_FRAME_LEN, tdiv * SCREEN_DIVS_X / ETS_FRAME_LEN as f64)
        }
    };
    let t0 = -(len as f64 / 2.0) * dt;
    adc_trace(number, &raw[..len], scale, t0, dt, probe, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic DPOX180H file with the real layout.
    pub(crate) struct Fake {
        pub ch1: Option<(u64, u16, Vec<u16>)>,
        pub ch2: Option<(u64, u16, Vec<u16>)>,
        pub rate: u32,
        pub tdiv_ps: u32,
    }

    impl Fake {
        pub fn bytes(&self) -> Vec<u8> {
            let settings_size = 1019u32;
            let screen_size = 11020u32;
            let mut wave = Vec::new();
            let mut count = 0;
            for (_, _, s) in self.ch1.iter().chain(self.ch2.iter()) {
                count = s.len() as u32;
                wave.extend(s.iter().flat_map(|v| v.to_be_bytes()));
            }
            let wave_start = 50 + settings_size + screen_size;
            let total = wave_start + wave.len() as u32;
            let mut f = vec![0u8; wave_start as usize];
            let put = |f: &mut Vec<u8>, at: usize, b: &[u8]| f[at..at + b.len()].copy_from_slice(b);
            for (i, v) in [
                50,
                settings_size,
                50 + settings_size,
                screen_size,
                wave_start,
                wave.len() as u32,
                total,
            ]
            .iter()
            .enumerate()
            {
                put(&mut f, i * 4, &v.to_be_bytes());
            }
            if let Some((uv, m, _)) = &self.ch1 {
                f[0x66] = 1;
                put(&mut f, 0x6E, &uv.to_be_bytes());
                put(&mut f, 0x7E, &m.to_be_bytes());
                put(&mut f, 0x82, &10_000u64.to_be_bytes());
            }
            if let Some((uv, m, _)) = &self.ch2 {
                f[0xA6] = 1;
                put(&mut f, 0xAE, &uv.to_be_bytes());
                put(&mut f, 0xBE, &m.to_be_bytes());
                put(&mut f, 0xC2, &100_000u64.to_be_bytes());
            }
            put(&mut f, 0x12D, &count.to_be_bytes());
            put(&mut f, 0x13D, &self.rate.to_be_bytes());
            put(&mut f, 0x1D9, &self.tdiv_ps.to_be_bytes());
            f.extend(wave);
            f
        }
    }

    fn real_time_fake() -> Fake {
        // 1 µs/div at 100 MS/s: 1200 samples on screen, 2400 in the buffer.
        Fake {
            ch1: Some((200_000, 100, vec![6400 + 1600; 2400])),
            ch2: Some((1_000_000, 50, vec![3200; 2400])),
            rate: 100_000_000,
            tdiv_ps: 1_000_000,
        }
    }

    fn trace<'a>(c: &'a Capture, label: &str) -> &'a Trace {
        c.trace(label).unwrap_or_else(|| panic!("no {label}"))
    }

    #[test]
    fn dual_channel_real_time() {
        let w = read(&real_time_fake().bytes()).unwrap();
        assert!(w.warnings.is_empty(), "{:?}", w.warnings);
        assert_eq!(
            w.acquisition,
            Acquisition::RealTime {
                sample_rate_hz: 1e8
            }
        );
        assert_eq!(w.record_length, 2400);
        let ch1 = trace(&w, "CH1");
        // One division above ground at 200 mV/div is +0.2 V.
        assert!((ch1.points()[0][1] - 0.2).abs() < 1e-6);
        // Trigger in the buffer centre: first sample at -12 µs.
        assert!((ch1.points()[0][0] + 12e-6).abs() < 1e-15);
        assert!((ch1.points()[1200][0]).abs() < 1e-15);
        let ch2 = trace(&w, "CH2");
        // Marker 50 px = 2 divisions below centre: 0 V is at -2 div.
        assert_eq!(ch2.display.zero_div, -2.0);
        assert_eq!(ch2.display.probe, 10.0);
        assert!(ch2.points().iter().all(|p| p[1].abs() < 1e-9));
    }

    #[test]
    fn ch2_follows_ch1_in_the_data_section() {
        let mut f = real_time_fake();
        f.ch1 = Some((1_000_000, 100, vec![6400; 2400]));
        f.ch2 = Some((1_000_000, 100, vec![8000; 2400]));
        let w = read(&f.bytes()).unwrap();
        assert!(trace(&w, "CH1").points().iter().all(|p| p[1] == 0.0));
        assert!(
            trace(&w, "CH2")
                .points()
                .iter()
                .all(|p| (p[1] - 1.0).abs() < 1e-6)
        );
    }

    #[test]
    fn only_ch2_enabled() {
        let mut f = real_time_fake();
        f.ch1 = None;
        let w = read(&f.bytes()).unwrap();
        assert!(w.trace("CH1").is_none());
        assert_eq!(trace(&w, "CH2").samples.len(), 2400);
    }

    #[test]
    fn equivalent_time_uses_first_frame() {
        let mut data: Vec<u16> = (0..3000).map(|i| (i % 300) as u16 * 10).collect();
        data[0] = 0; // clipped sample inside frame 0
        let f = Fake {
            ch1: Some((100_000, 100, data)),
            ch2: None,
            rate: 705_032_704,
            tdiv_ps: 5_000,
        };
        let w = read(&f.bytes()).unwrap();
        assert_eq!(w.acquisition, Acquisition::Equivalent { frames: 10 });
        let ch1 = trace(&w, "CH1");
        assert_eq!(ch1.points().len(), 300);
        // 12 divisions x 5 ns = 60 ns, centred on the trigger.
        assert!((ch1.points()[0][0] + 30e-9).abs() < 1e-18);
        assert!((ch1.points()[150][0]).abs() < 1e-18);
        assert!(ch1.clipped());
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(read(&[0; 10]), Err(ParseError::TooShort(10)));
        let mut riff = real_time_fake().bytes();
        riff[..4].copy_from_slice(b"RIFF");
        assert_eq!(DpoxFormat.probe(&riff), Confidence::No);
        assert!(matches!(read(&riff), Err(ParseError::BadSectionTable(_))));
        let mut truncated = real_time_fake().bytes();
        truncated.pop();
        assert_eq!(DpoxFormat.probe(&truncated), Confidence::No);
        assert!(matches!(
            read(&truncated),
            Err(ParseError::BadSectionTable(_))
        ));
        let mut off = real_time_fake();
        off.ch1 = None;
        off.ch2 = None;
        assert_eq!(read(&off.bytes()), Err(ParseError::NoChannels));
        assert_eq!(
            DpoxFormat.probe(&real_time_fake().bytes()),
            Confidence::Sure
        );
    }

    #[test]
    fn sample_count_mismatch_is_a_warning() {
        let mut b = real_time_fake().bytes();
        b[0x12D..0x131].copy_from_slice(&999u32.to_be_bytes());
        let w = read(&b).unwrap();
        assert_eq!(w.record_length, 2400);
        assert_eq!(w.warnings.len(), 1);
    }
}
