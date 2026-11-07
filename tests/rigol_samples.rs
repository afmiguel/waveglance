//! Tests with real Rigol files (RigolWFM project, BSD-3). Run
//! `scripts/fetch-samples.sh` first.
//!
//! The scopes have a 1 kHz calibration output; most samples show it.

use fnirsi_viewer::formats;
use fnirsi_viewer::measure::{Measurement, measure};
use fnirsi_viewer::model::Capture;
use std::path::PathBuf;

fn bytes(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/rigol")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}\nRun scripts/fetch-samples.sh to download the sample files.",
            path.display()
        )
    })
}

fn load(name: &str) -> Capture {
    formats::open(&bytes(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn frequency(w: &Capture, i: usize) -> f64 {
    let t = &w.traces[i];
    let span = t.time_span().unwrap();
    measure(t.points(), &span, 0.1 * t.display.per_div)
        .get(Measurement::Frequency)
        .unwrap()
}

#[test]
fn bin_files() {
    // (file, model, channels, time/div)
    for (name, model, channels, tdiv) in [
        ("DHO824-ch1.bin", "DHO824", 1, 400e-6),
        ("DHO824-ch12.bin", "DHO824", 2, 400e-6),
        ("DHO824-ch1234.bin", "DHO824", 4, 400e-6),
        ("MSO5000-A.bin", "MSO5XXX", 4, 500e-6),
        ("MSO5000-B.bin", "MSO50XX", 4, 10e-3),
    ] {
        let w = load(name);
        assert_eq!(w.source.vendor, "Rigol", "{name}");
        assert_eq!(w.source.model, model, "{name}");
        assert_eq!(w.traces.len(), channels, "{name}");
        let t = w.time_per_div.unwrap();
        assert!((t - tdiv).abs() < tdiv * 1e-6, "{name}: {t}");
        // The trigger (t = 0) is at the centre of the record.
        let span = w.time_span().unwrap();
        assert!(
            (span.start() + span.end()).abs() < 0.01 * tdiv * 10.0,
            "{name}: {span:?}"
        );
        let f = frequency(&w, 0);
        assert!((f - 1000.0).abs() < 1.0, "{name}: f = {f}");
    }
}

#[test]
fn damaged_bin_files_are_refused() {
    // Old MSO5074 files: samples like 1e33.
    for name in ["MSO5074-A.bin", "MSO5074-B.bin"] {
        assert!(formats::open(&bytes(name)).is_err(), "{name}");
    }
}

#[test]
fn csv_files() {
    // Rigol CSV: "X,CH1,…,Start,Increment" with Start and Increment in the
    // units line. (file, channels, first time, frequency)
    for (name, channels, t0, f_expected) in [
        ("DS1054Z-C.csv", 4, -6e-3, 1000.0),
        ("DS1054Z-D.csv", 1, -3.22e-3, 1000.0),
        ("DS2072A-1.csv", 2, -3.5e-3, 1000.0),
        ("DS2072A-4.csv", 2, -7e-2, 1000.0),
        ("DS2072A-9.csv", 1, -3.5e-3, 500.0),
    ] {
        let w = load(name);
        assert_eq!(w.traces.len(), channels, "{name}");
        let start = *w.time_span().unwrap().start();
        assert!((start - t0).abs() < t0.abs() * 1e-3, "{name}: {start}");
        let f = frequency(&w, 0);
        assert!(
            (f - f_expected).abs() < f_expected * 0.01,
            "{name}: f = {f}"
        );
    }
}
