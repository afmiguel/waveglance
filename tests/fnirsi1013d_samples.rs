//! Tests with real FNIRSI 1013D files. Run `scripts/fetch-samples.sh` first.
//!
//! Expected values were read from the screenshots of the same captures
//! (lexus2026HA/FNIRSI-2C53T-Decoder, tests/test_1013d.py).

use std::path::PathBuf;
use waveglance::formats;
use waveglance::measure::{Measurement, measure};
use waveglance::model::{Capture, Coupling};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/fnirsi1013d")
}

fn load(n: u32) -> Capture {
    let path = data_dir().join(format!("{n}.wav"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}\nRun scripts/fetch-samples.sh to download the sample files.",
            path.display()
        )
    });
    formats::open(&bytes).unwrap_or_else(|e| panic!("{n}.wav: {e}"))
}

#[test]
fn settings_match_the_screenshots() {
    // file: (time/div, CH1 V/div, CH1 coupling, CH2 on, CH2 V/div)
    let truth = [
        (1, 500e-6, 1.0, Coupling::Dc, true, 0.05),
        (2, 100e-6, 0.5, Coupling::Dc, true, 0.05),
        (3, 2e-3, 0.05, Coupling::Ac, false, 5.0),
        (12, 2.0, 1.0, Coupling::Ac, false, 5.0),
        (19, 5e-6, 0.5, Coupling::Ac, false, 50.0),
        (20, 5e-6, 0.5, Coupling::Ac, false, 50.0),
        (21, 5e-6, 0.5, Coupling::Ac, false, 50.0),
    ];
    for (n, tdiv, v1, c1, ch2_on, v2) in truth {
        let w = load(n);
        assert_eq!(w.source.model, "1013D/1014D", "{n}");
        assert_eq!(w.record_length, 1500, "{n}");
        assert!((w.time_per_div.unwrap() - tdiv).abs() < tdiv * 1e-9, "{n}");
        let ch1 = w.trace("CH1").unwrap();
        assert!((ch1.display.per_div - v1).abs() < 1e-12, "{n}");
        assert_eq!(ch1.display.coupling, Some(c1), "{n}");
        assert_eq!(w.trace("CH2").is_some(), ch2_on, "{n}");
        if let Some(ch2) = w.trace("CH2") {
            assert!((ch2.display.per_div - v2).abs() < 1e-12, "{n}");
        }
    }
}

#[test]
fn values_match_the_scope_readout() {
    // Screen of file 1: CH1 Vpp 3.17 V, Freq 1.00 kHz (1 V/div DC, 500 µs/div).
    let w = load(1);
    let ch1 = w.trace("CH1").unwrap();
    let all = ch1.points()[0][0]..=ch1.points().last().unwrap()[0];
    let r = measure(ch1.points(), &all, 0.1 * ch1.display.per_div);
    let vpp = r.get(Measurement::Vpp).unwrap();
    assert!((vpp - 3.17).abs() < 0.1, "Vpp = {vpp}");
    let f = r.get(Measurement::Frequency).unwrap();
    assert!((f - 1000.0).abs() < 5.0, "f = {f}");
    // Buffer = 30 divisions, trigger at the centre.
    assert!((all.start() + 15.0 * 500e-6).abs() < 1e-9, "{all:?}");
}

#[test]
fn trigger_is_at_the_buffer_centre() {
    // Rising-edge trigger: the signal crosses its mid level near t = 0.
    for n in [1, 8, 13, 15, 17, 18] {
        let w = load(n);
        let t = w.trigger.as_ref().unwrap();
        assert!(t.rising && t.source.as_deref() == Some("CH1"), "{n}");
        let pts = w.trace("CH1").unwrap().points();
        let (lo, hi) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            (lo.min(p[1]), hi.max(p[1]))
        });
        let mid = (lo + hi) / 2.0;
        let dt = pts[1][0] - pts[0][0];
        let nearest = pts
            .windows(2)
            .filter(|p| p[0][1] < mid && p[1][1] >= mid)
            .map(|p| p[1][0].abs())
            .fold(f64::MAX, f64::min);
        assert!(
            nearest <= 3.0 * dt,
            "{n}: nearest rising crossing {nearest}, dt {dt}"
        );
    }
}

#[test]
fn all_1013d_files_parse() {
    let mut count = 0;
    for entry in std::fs::read_dir(data_dir()).expect("run scripts/fetch-samples.sh first") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "wav") {
            let w = formats::open(&std::fs::read(&path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(w.source.model, "1013D/1014D");
            count += 1;
        }
    }
    assert_eq!(count, 21);
}
