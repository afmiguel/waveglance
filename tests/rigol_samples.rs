//! Tests with real Rigol files (RigolWFM project, BSD-3). Run
//! `scripts/fetch-samples.sh` first.
//!
//! The scopes have a 1 kHz calibration output; most samples show it.

use fnirsi_viewer::formats;
use fnirsi_viewer::measure::{Measurement, measure};
use fnirsi_viewer::model::{Capture, Samples, Trace};
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
        let labels: Vec<&str> = w.traces.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["CH1", "CH2", "CH3", "CH4"][..channels], "{name}");
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

#[test]
fn wfm_files_open() {
    // (file, model, channels, V/div of the first channel, time/div)
    for (name, model, labels, vdiv, tdiv) in [
        (
            "DS1054Z-B.wfm",
            "DS1054Z",
            &["CH1", "CH2", "CH3", "CH4"][..],
            2.0,
            100e-6,
        ),
        (
            "DS1054Z-ch1SquareCH4Uart.wfm",
            "DS1104Z",
            &["CH1", "CH4"][..],
            1.0,
            200e-6,
        ),
        (
            "DS1074Z-A.wfm",
            "DS1074Z",
            &["CH1", "CH2", "CH3"][..],
            0.05,
            2e-3,
        ),
        ("MSO1104.wfm", "MSO1104Z", &["CH1"][..], 5.0, 1e-3),
        ("DS1202Z-E.wfm", "DS1202Z-E", &["CH1"][..], 1.0, 1.0),
        (
            "DS1102E-D.wfm",
            "DS1000E/D",
            &["CH1", "CH2"][..],
            2.0,
            100e-6,
        ),
        (
            "DS1102D-A.wfm",
            "DS1000E/D",
            &["CH1", "CH2"][..],
            1.0,
            500e-6,
        ),
        ("DS1052E.wfm", "DS1000E/D", &["CH1", "CH2"][..], 1.0, 100e-9),
        ("DS2072A-1.wfm", "DS2000", &["CH1", "CH2"][..], 0.2, 500e-6),
        ("DS2072A-7.wfm", "DS2000", &["CH2"][..], 0.2, 500e-6),
        ("DS2202.wfm", "DS2000", &["CH1", "CH2"][..], 10.0, 5e-3),
    ] {
        let w = load(name);
        assert_eq!(w.source.vendor, "Rigol", "{name}");
        assert_eq!(w.source.model, model, "{name}");
        let got: Vec<&str> = w.traces.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(got, labels, "{name}");
        assert_eq!(w.traces[0].display.per_div, vdiv, "{name}");
        let t = w.time_per_div.unwrap();
        assert!((t - tdiv).abs() < tdiv * 1e-6, "{name}: {t}");
    }
    // Only logic analyzer data: no analog channel is on.
    assert!(formats::open(&bytes("DS1074Z-C.wfm")).is_err());
}

/// Samples of `w` at the times of the CSV points of `c`: the mean
/// difference in LSB and the share of points within 1 LSB.
fn pointwise(w: &Trace, c: &Trace) -> (f64, f64) {
    let Samples::Uniform { t0, dt, values } = &w.samples else {
        panic!("uniform samples expected")
    };
    let (mut n, mut sum, mut close) = (0usize, 0.0, 0usize);
    for p in c.points() {
        let i = ((p[0] - t0) / dt).round();
        let Some(&v) = (i >= 0.0).then(|| values.get(i as usize)).flatten() else {
            continue;
        };
        let d = (f64::from(v) - p[1]).abs() / w.lsb();
        n += 1;
        sum += d;
        close += usize::from(d <= 1.01);
    }
    assert!(
        n * 10 >= c.samples.len() * 9,
        "the CSV times are outside the record"
    );
    (sum / n as f64, close as f64 / n as f64)
}

#[test]
fn wfm_samples_match_the_scope_csv() {
    // DS1000E and DS2000: the CSV saved by the scope holds samples of the
    // same record. (file, mean difference limit in LSB)
    for (name, limit) in [
        ("DS1102E-B", 2.0),
        ("DS1102E-D", 1.0),
        // DS1102D-A: gain and 0 V match the CSV within 2 codes, but the
        // CSV times are about 2 samples away and the signal changes fast
        // at 100 kSa/s, so the pointwise difference is larger.
        ("DS1102D-A", 8.0),
        ("DS2072A-1", 2.0),
        ("DS2072A-3", 2.0),
        ("DS2072A-5", 2.0),
        ("DS2072A-6", 2.0),
        ("DS2072A-7", 2.0),
        ("DS2072A-9", 0.1),
    ] {
        let w = load(&format!("{name}.wfm"));
        let c = load(&format!("{name}.csv"));
        assert_eq!(w.traces.len(), c.traces.len(), "{name}");
        for (wt, ct) in w.traces.iter().zip(&c.traces) {
            let (mean, _) = pointwise(wt, ct);
            assert!(mean < limit, "{name} {}: {mean:.2} LSB", wt.label);
        }
    }
}

/// The `count` samples in the middle of `v`.
fn centre(v: &[f32], count: usize) -> &[f32] {
    let start = v.len().saturating_sub(count) / 2;
    &v[start..start + count.min(v.len())]
}

fn values(t: &Trace) -> Vec<f32> {
    match &t.samples {
        Samples::Uniform { values, .. } => values.clone(),
        Samples::Timed(p) => p.iter().map(|p| p[1] as f32).collect(),
    }
}

#[test]
fn ds1000z_levels_match_the_scope_csv() {
    // The DS1000Z CSV files do not hold the same samples as the .wfm, so
    // (like RigolWFM) the sorted values are compared: same levels and
    // distribution. (file, channel index in the .wfm and in the CSV)
    for (name, i) in [
        ("DS1054Z-A", 0),
        ("DS1054Z-A", 1),
        ("DS1054Z-A", 2),
        ("DS1054Z-B", 0),
        ("DS1054Z-B", 1),
        ("DS1054Z-B", 2),
        ("DS1054Z-B", 3),
        ("DS1054Z-C", 0),
        ("DS1054Z-C", 2),
    ] {
        let w = values(&load(&format!("{name}.wfm")).traces[i]);
        let c = values(&load(&format!("{name}.csv")).traces[i]);
        let n = w.len().min(c.len());
        let mut a = centre(&w, n).to_vec();
        let mut b = centre(&c, n).to_vec();
        a.sort_by(f32::total_cmp);
        b.sort_by(f32::total_cmp);
        let d: Vec<f64> = a.iter().zip(&b).map(|(x, y)| f64::from(x - y)).collect();
        let mean = d.iter().sum::<f64>() / n as f64;
        let rms = (d.iter().map(|x| x * x).sum::<f64>() / n as f64).sqrt();
        let max = d.iter().fold(0.0f64, |m, x| m.max(x.abs()));
        assert!(
            mean.abs() < 0.4 && rms < 0.5 && max < 0.6,
            "{name} CH{}: mean {mean:.3} rms {rms:.3} max {max:.3}",
            i + 1
        );
    }
    // Older firmware with two channels: the DC level of the first 1000
    // points.
    for name in ["DS1054Z-ch1SquareCH2Uart", "DS1054Z-ch1SquareCH4Uart"] {
        let w = load(&format!("{name}.wfm"));
        let c = load(&format!("{name}.csv"));
        for i in 0..2 {
            let cv = values(&c.traces[i]);
            let wv = values(&w.traces[i]);
            let a = centre(&wv, cv.len());
            let err: f64 = a[..1000]
                .iter()
                .zip(&cv[..1000])
                .map(|(x, y)| f64::from(x - y))
                .sum::<f64>()
                / 1000.0;
            assert!(err.abs() < 0.06, "{name} CH{}: {err:.3}", i + 1);
        }
    }
}
