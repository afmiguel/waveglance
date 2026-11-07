//! Tests with real Keysight / Agilent `.bin` files. Run `scripts/fetch-samples.sh` first.
//!
//! Expected values come from the `.txt` file that the scope saved with each
//! capture (settings and its own measurements).

use fnirsi_viewer::formats;
use fnirsi_viewer::measure::{Measurement, measure};
use fnirsi_viewer::model::{Capture, Unit};
use std::path::PathBuf;

fn load(name: &str) -> Capture {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/keysight")
        .join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}\nRun scripts/fetch-samples.sh to download the sample files.",
            path.display()
        )
    });
    assert_eq!(
        formats::detect(&bytes).first().map(|f| f.0.name()),
        Some("Keysight / Agilent / Rigol (.bin)"),
        "{name}"
    );
    formats::open(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Frequency and Vpp of trace `i` over the whole record.
fn freq_vpp(w: &Capture, i: usize) -> (f64, f64) {
    let t = &w.traces[i];
    let span = t.time_span().unwrap();
    let r = measure(t.points(), &span, 0.1 * t.display.per_div);
    (
        r.get(Measurement::Frequency).unwrap(),
        r.get(Measurement::Vpp).unwrap(),
    )
}

#[test]
fn settings_match_the_scope() {
    // (file, time/div from the .txt, channels)
    for (name, tdiv, channels) in [
        ("agilent_1.bin", 100e-6, 1),
        ("agilent_2.bin", 2e-6, 1),
        ("agilent_3.bin", 200e-9, 2),
        ("agilent_4.bin", 200e-6, 1),
    ] {
        let w = load(name);
        assert_eq!(w.source.model, "DSO-X 1102G", "{name}");
        let t = w.time_per_div.unwrap();
        assert!((t - tdiv).abs() < tdiv * 1e-6, "{name}: {t}");
        assert_eq!(w.traces.len(), channels, "{name}");
    }
    // agilent_2 also holds the EXT trigger input as a digital buffer: not shown.
    assert!(
        load("agilent_2.bin")
            .warnings
            .iter()
            .any(|w| w.contains("digital"))
    );
}

#[test]
fn measurements_match_the_scope() {
    // (file, scope frequency, scope Vpp) from the MEASUREMENTS of the .txt.
    // agilent_2: same frequency as the screenshot, but the samples are 10
    // times larger than its 2.73 Vpp (the .bin was probably saved with
    // another probe setting); only its frequency is checked.
    for (name, f_scope, vpp_scope) in [
        ("agilent_2.bin", 249.69e3, None),
        ("agilent_3.bin", 998.0e3, Some(5.6)),
        ("agilent_4.bin", 1.0e3, Some(1.00)),
    ] {
        let (f, vpp) = freq_vpp(&load(name), 0);
        assert!((f - f_scope).abs() < f_scope * 0.01, "{name}: f = {f}");
        if let Some(v) = vpp_scope {
            assert!((vpp - v).abs() < v * 0.05 + 0.02, "{name}: Vpp = {vpp}");
        }
    }
}

#[test]
fn four_channels_with_current() {
    // MSO-X 4154A: 4 channels of 2 million points; CH3 and CH4 in amperes.
    let w = load("agilent_msox4154a_01.bin");
    assert_eq!(w.source.model, "MSO-X 4154A");
    let labels: Vec<&str> = w.traces.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(labels, ["CH1", "CH2", "CH3", "CH4"]);
    let units: Vec<Unit> = w.traces.iter().map(|t| t.unit).collect();
    assert_eq!(units, [Unit::Volt, Unit::Volt, Unit::Ampere, Unit::Ampere]);
    assert_eq!(w.record_length, 2_000_000);
}

#[test]
fn long_record() {
    let w = load("agilent_5.bin");
    assert_eq!(w.traces[0].samples.len(), 999_999);
}
