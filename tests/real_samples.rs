//! Tests with real DPOX180H files. Run `scripts/fetch-samples.sh` first.
//!
//! Expected values come from the screen of the scope at save time (the
//! `*_src.png` screenshots in the source repository and the thumbnail
//! that each file contains).

use fnirsi_viewer::parser::{self, Acquisition, Channel, Model, Waveform};
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/dpox180h")
}

fn load(name: &str) -> Waveform {
    let path = data_dir().join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}\nRun scripts/fetch-samples.sh to download the sample files.",
            path.display()
        )
    });
    parser::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn vpp(ch: &Channel) -> f64 {
    let (lo, hi) = ch.points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p[1]), hi.max(p[1]))
    });
    hi - lo
}

fn mean(ch: &Channel) -> f64 {
    ch.points.iter().map(|p| p[1]).sum::<f64>() / ch.points.len() as f64
}

/// Times of rising crossings through the mid level, with hysteresis.
fn rising_edges(ch: &Channel) -> Vec<f64> {
    let (lo, hi) = ch.points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p[1]), hi.max(p[1]))
    });
    let (mid, band) = ((lo + hi) / 2.0, (hi - lo) * 0.2);
    let mut armed = false;
    let mut edges = Vec::new();
    for p in &ch.points {
        if p[1] < mid - band {
            armed = true;
        } else if armed && p[1] > mid + band {
            edges.push(p[0]);
            armed = false;
        }
    }
    edges
}

#[test]
fn square_100khz_real_time() {
    // Screen: CH1 200 mV/div, CH2 5 mV/div (off), H = 5 µs, F 99.98 kHz, Vpp 1.07 V.
    let w = load("100kHZ-1V.wav");
    assert!(w.warnings.is_empty(), "{:?}", w.warnings);
    assert_eq!(w.model, Model::Dpox180h);
    assert!(w.trigger.is_none());
    assert_eq!(w.samples_per_channel, 48_000);
    assert_eq!(
        w.acquisition,
        Acquisition::RealTime {
            sample_rate_hz: 400e6
        }
    );
    assert!((w.time_per_div_s - 5e-6).abs() < 1e-15);
    assert!(w.ch2.is_none());

    let ch1 = w.ch1.as_ref().unwrap();
    assert!((ch1.volts_per_div - 0.2).abs() < 1e-12);
    assert_eq!(ch1.probe_factor, 1.0);
    assert!((vpp(ch1) - 1.07).abs() < 0.05, "Vpp = {}", vpp(ch1));
    // Symmetric square wave around ground: mean close to 0 V.
    assert!(mean(ch1).abs() < 0.03, "mean = {}", mean(ch1));

    // Buffer = 2 screens (2 x 12 div x 5 µs) centred on the trigger.
    let (t0, t1) = (ch1.points[0][0], ch1.points.last().unwrap()[0]);
    assert!(
        (t0 + 60e-6).abs() < 1e-9 && (t1 - 60e-6).abs() < 1e-8,
        "{t0} .. {t1}"
    );

    // Period close to 10 µs (scope frequency counter: 99.98 kHz).
    // Median gap between edges, so that ringing does not count.
    let e = rising_edges(ch1);
    let mut gaps: Vec<f64> = e.windows(2).map(|p| p[1] - p[0]).collect();
    gaps.sort_by(f64::total_cmp);
    let period = gaps[gaps.len() / 2];
    assert!((period - 10e-6).abs() < 0.2e-6, "period = {period}");

    // Rising-edge trigger: one rising edge is at t = 0 (within 1% of the screen).
    let nearest = e.iter().map(|t| t.abs()).fold(f64::MAX, f64::min);
    assert!(nearest < 0.6e-6, "nearest edge to t=0 is {nearest}");
}

#[test]
fn sine_10mhz_equivalent_time() {
    // Screen: CH1 200 mV/div, CH2 1 V/div (off), H = 5 ns, F 10 MHz, Vpp 968 mV.
    let w = load("10MHz-1V.wav");
    assert_eq!(w.samples_per_channel, 3000);
    assert_eq!(w.acquisition, Acquisition::Equivalent { frames: 10 });
    assert!((w.time_per_div_s - 5e-9).abs() < 1e-18);

    let ch1 = w.ch1.as_ref().unwrap();
    assert!((ch1.volts_per_div - 0.2).abs() < 1e-12);
    assert_eq!(ch1.points.len(), 300);
    // 60 ns of a 100 ns period: the frame shows about 0.6 of the sine.
    assert!(vpp(ch1) > 0.8 && vpp(ch1) < 1.0, "Vpp = {}", vpp(ch1));
    // Rising slope through the trigger at the screen centre.
    let mid = &ch1.points[140..160];
    assert!(mid.windows(2).all(|p| p[1][1] >= p[0][1] - 0.01));
    assert!(
        ch1.points[150][1].abs() < 0.1,
        "v(0) = {}",
        ch1.points[150][1]
    );
}

#[test]
fn open_input_is_zero_volts() {
    // 1 V/div with the probe set to 10X: noise only, near 0 V.
    let w = load("Waveform-google/open.wav");
    let ch1 = w.ch1.as_ref().unwrap();
    assert_eq!(ch1.probe_factor, 10.0);
    assert!((ch1.volts_per_div - 1.0).abs() < 1e-12);
    let div = ch1.volts_per_div;
    assert!(mean(ch1).abs() < 0.05 * div, "mean = {}", mean(ch1));
    assert!(vpp(ch1) < 0.1 * div, "Vpp = {}", vpp(ch1));
}

#[test]
fn probe_10x_reads_ten_times_the_1x_voltage() {
    // Same 10 MHz signal: 1X at 200 mV/div, and 10X at 2 V/div.
    // The ADC codes are the same; the probe-tip voltage is 10 times larger.
    let x1 = load("10MHz-1V.wav");
    let x10 = load("Waveform-google/10MHz-1V.wav");
    let (a, b) = (x1.ch1.as_ref().unwrap(), x10.ch1.as_ref().unwrap());
    assert_eq!((a.probe_factor, b.probe_factor), (1.0, 10.0));
    assert!((a.volts_per_div / a.probe_factor - b.volts_per_div / b.probe_factor).abs() < 1e-12);
    // Two separate captures, and a 60 ns frame shows only part of the
    // 100 ns period, so the visible Vpp changes with the phase: allow ±10%.
    let ratio = vpp(b) / vpp(a);
    assert!((ratio - 10.0).abs() < 1.0, "ratio = {ratio}");
}

#[test]
fn all_sample_files_parse() {
    let mut count = 0;
    for dir in [data_dir(), data_dir().join("Waveform-google")] {
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| {
            panic!(
                "cannot read {}: {e}\nRun scripts/fetch-samples.sh first.",
                dir.display()
            )
        });
        for entry in entries {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "wav") {
                let w = parser::parse(&std::fs::read(&path).unwrap())
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                assert!(w.ch1.is_some(), "{}", path.display());
                count += 1;
            }
        }
    }
    assert_eq!(count, 23, "expected 23 sample files");
}

#[test]
fn measurements_of_the_100khz_square() {
    // Scope screen: F 99.98 kHz, Vpp 1.07 V. The signal has short glitches.
    use fnirsi_viewer::measure::{Measurement, measure};
    let w = load("100kHZ-1V.wav");
    let ch1 = w.ch1.as_ref().unwrap();
    for window in [-60e-6..=60e-6, -30e-6..=30e-6] {
        let r = measure(&ch1.points, &window, 0.1 * ch1.volts_per_div);
        let f = r.get(Measurement::Frequency).unwrap();
        assert!((f - 99.98e3).abs() < 1e3, "{window:?}: f = {f}");
        let duty = r.get(Measurement::DutyCycle).unwrap();
        assert!((duty - 50.0).abs() < 5.0, "{window:?}: duty = {duty}");
        let vamp = r.get(Measurement::Vamp).unwrap();
        assert!((vamp - 0.94).abs() < 0.1, "{window:?}: Vamp = {vamp}");
    }
}
