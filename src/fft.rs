//! Spectrum (FFT) of a channel, like the FFT math function of a scope.
//!
//! - The samples are multiplied by a window function, padded with zeros to a
//!   power of two, and transformed with a radix-2 FFT.
//! - The result is the single-sided amplitude spectrum in volts RMS: a sine of
//!   amplitude A at a bin frequency reads A/√2. The window gain is corrected
//!   (coherent gain), so the level does not depend on the window, except for
//!   the scalloping loss between bins (up to 1.4 dB with Hanning, 3.9 dB with
//!   Rectangular, below 0.1 dB with Flat top).

use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, SQRT_2, TAU};

/// The most samples used: a longer window uses its first samples only (the
/// app says so in its N readout).
pub const MAX_SAMPLES: usize = 1 << 20;
/// Points at which a trace with uneven times (logic signals, some CSV files)
/// is sampled for the FFT.
pub const RESAMPLE_POINTS: usize = 1 << 16;

/// Window function applied before the FFT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Window {
    /// No window. Best for transients and for frequencies that fit the record exactly.
    Rectangular,
    /// General use with periodic signals.
    #[default]
    Hanning,
    /// Like Hanning, slightly narrower peak.
    Hamming,
    /// Low leakage: shows small harmonics next to a large peak.
    BlackmanHarris,
    /// Accurate amplitude, wide peak.
    FlatTop,
}

impl Window {
    pub const ALL: [Self; 5] = [
        Self::Rectangular,
        Self::Hanning,
        Self::Hamming,
        Self::BlackmanHarris,
        Self::FlatTop,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Rectangular => "Rectangular",
            Self::Hanning => "Hanning",
            Self::Hamming => "Hamming",
            Self::BlackmanHarris => "Blackman-Harris",
            Self::FlatTop => "Flat top",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Rectangular => "No window: transients, or a whole number of periods",
            Self::Hanning => "General use with periodic signals",
            Self::Hamming => "Like Hanning, slightly narrower peak",
            Self::BlackmanHarris => "Low leakage: small harmonics next to a large peak",
            Self::FlatTop => "Accurate amplitude, wide peak",
        }
    }

    /// Cosine-sum coefficients a0, a1, a2, a3, a4.
    fn coefficients(self) -> [f64; 5] {
        match self {
            Self::Rectangular => [1.0, 0.0, 0.0, 0.0, 0.0],
            Self::Hanning => [0.5, 0.5, 0.0, 0.0, 0.0],
            Self::Hamming => [0.54, 0.46, 0.0, 0.0, 0.0],
            Self::BlackmanHarris => [0.35875, 0.48829, 0.14128, 0.01168, 0.0],
            Self::FlatTop => [
                0.215_578_95,
                0.416_631_58,
                0.277_263_158,
                0.083_578_947,
                0.006_947_368,
            ],
        }
    }

    /// The window value for sample `i` of `n`.
    pub fn value(self, i: usize, n: usize) -> f64 {
        if n < 2 {
            return 1.0;
        }
        let x = TAU * i as f64 / (n - 1) as f64;
        let a = self.coefficients();
        a[0] - a[1] * x.cos() + a[2] * (2.0 * x).cos() - a[3] * (3.0 * x).cos()
            + a[4] * (4.0 * x).cos()
    }
}

/// A single-sided amplitude spectrum.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Spectrum {
    /// Frequency of each bin, in Hz.
    pub freqs: Vec<f64>,
    /// Level of each bin, in volts RMS (the DC bin: the mean value).
    pub vrms: Vec<f64>,
    /// The window used.
    pub window: Window,
}

impl Spectrum {
    /// The highest line above DC (bins 0 and 1 are skipped, because the DC
    /// level leaks into bin 1 with most windows): (frequency, volts RMS).
    ///
    /// The frequency is interpolated between bins (a parabola through the
    /// logarithms of the three highest bins), so it is finer than the bin
    /// step. Not with the Flat top window: its peak is flat over several
    /// bins, so the parabola does not fit. The level is the level of the
    /// highest bin.
    pub fn peak(&self) -> Option<(f64, f64)> {
        let i = (2..self.vrms.len()).max_by(|&a, &b| self.vrms[a].total_cmp(&self.vrms[b]))?;
        let df = self.resolution().unwrap_or(0.0);
        let mut f = self.freqs[i];
        if self.window != Window::FlatTop
            && let (Some(&a), Some(&c)) = (self.vrms.get(i - 1), self.vrms.get(i + 1))
        {
            let b = self.vrms[i];
            if a > 0.0 && b > 0.0 && c > 0.0 {
                let (la, lb, lc) = (a.ln(), b.ln(), c.ln());
                let denominator = la - 2.0 * lb + lc;
                if denominator < 0.0 {
                    let delta = 0.5 * (la - lc) / denominator;
                    f += delta.clamp(-0.5, 0.5) * df;
                }
            }
        }
        Some((f, self.vrms[i]))
    }

    /// Frequency step between bins, in Hz.
    pub fn resolution(&self) -> Option<f64> {
        Some(self.freqs.get(1)? - self.freqs.first()?)
    }
}

/// Volts RMS to dBV (0 dBV = 1 V RMS). Very small levels are limited to −200 dBV.
pub fn dbv(vrms: f64) -> f64 {
    20.0 * vrms.max(1e-10).log10()
}

/// Spectrum of evenly spaced samples `values`, `dt` seconds apart.
/// `None` if there are fewer than 4 samples or `dt` is not positive.
pub fn spectrum(values: &[f64], dt: f64, window: Window) -> Option<Spectrum> {
    let values = &values[..values.len().min(MAX_SAMPLES)];
    let n = values.len();
    if n < 4 || dt <= 0.0 || !dt.is_finite() {
        return None;
    }
    let size = n.next_power_of_two();
    let mut re = vec![0.0; size];
    let mut im = vec![0.0; size];
    let mut gain = 0.0;
    for (i, &v) in values.iter().enumerate() {
        let w = window.value(i, n);
        re[i] = v * w;
        gain += w;
    }
    fft_in_place(&mut re, &mut im);

    let bins = size / 2 + 1;
    let df = 1.0 / (size as f64 * dt);
    let mut out = Spectrum {
        freqs: Vec::with_capacity(bins),
        vrms: Vec::with_capacity(bins),
        window,
    };
    for k in 0..bins {
        let magnitude = re[k].hypot(im[k]) / gain;
        // DC: the mean. Other bins: 2 × |X| (single-sided) / √2 (RMS).
        let level = if k == 0 {
            magnitude
        } else {
            magnitude * SQRT_2
        };
        out.freqs.push(k as f64 * df);
        out.vrms.push(level);
    }
    Some(out)
}

/// Iterative radix-2 FFT. The length must be a power of two.
fn fft_in_place(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    // Bit-reversal permutation.
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * PI / len as f64;
        let (w_re, w_im) = (angle.cos(), angle.sin());
        for start in (0..n).step_by(len) {
            let (mut c_re, mut c_im) = (1.0, 0.0);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let t_re = re[b] * c_re - im[b] * c_im;
                let t_im = re[b] * c_im + im[b] * c_re;
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
                let next = c_re * w_re - c_im * w_im;
                c_im = c_re * w_im + c_im * w_re;
                c_re = next;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `n` samples of offset + amplitude × sin(2π f t), sampled every `dt`.
    fn sine(n: usize, dt: f64, f: f64, amplitude: f64, offset: f64) -> Vec<f64> {
        (0..n)
            .map(|i| offset + amplitude * (TAU * f * i as f64 * dt).sin())
            .collect()
    }

    #[test]
    fn sine_on_a_bin_reads_its_rms_value() {
        // 4096 samples at 1 MS/s: bin step 244.14 Hz; bin 41 = 10.010 kHz.
        let dt = 1e-6;
        let f = 41.0 / (4096.0 * dt);
        let x = sine(4096, dt, f, 2.0, 0.0);
        for window in Window::ALL {
            let s = spectrum(&x, dt, window).unwrap();
            let (pf, pv) = s.peak().unwrap();
            assert!(
                (pf - f).abs() < 1e-3 / (4096.0 * dt),
                "{window:?}: peak at {pf}"
            );
            assert!((pv - SQRT_2).abs() < 0.01, "{window:?}: {pv} V RMS");
        }
    }

    #[test]
    fn dc_and_resolution() {
        let x = vec![0.5; 1000];
        let s = spectrum(&x, 1e-3, Window::Rectangular).unwrap();
        assert!((s.vrms[0] - 0.5).abs() < 1e-12, "DC = mean");
        // Padded to 1024 samples of 1 ms: 0.9766 Hz per bin.
        assert!((s.resolution().unwrap() - 1.0 / 1.024).abs() < 1e-12);
        assert_eq!(s.freqs.len(), 513);
    }

    #[test]
    fn flat_top_amplitude_between_bins() {
        // A frequency half-way between two bins: Flat top stays within 0.1 dB.
        let dt = 1e-6;
        let f = 40.5 / (4096.0 * dt);
        let x = sine(4096, dt, f, 1.0, 0.0);
        let s = spectrum(&x, dt, Window::FlatTop).unwrap();
        let (_, pv) = s.peak().unwrap();
        assert!((dbv(pv) - dbv(1.0 / SQRT_2)).abs() < 0.1, "{} dBV", dbv(pv));
    }

    #[test]
    fn peak_frequency_between_bins() {
        // 10.3 bins: the peak frequency is found within 0.05 bin.
        let dt = 1e-6;
        let df = 1.0 / (4096.0 * dt);
        let f = 10.3 * df;
        let x = sine(4096, dt, f, 1.0, 0.0);
        let (pf, _) = spectrum(&x, dt, Window::Hanning).unwrap().peak().unwrap();
        assert!((pf - f).abs() < 0.05 * df, "{pf} vs {f}");
    }

    #[test]
    fn too_few_samples() {
        assert_eq!(spectrum(&[1.0, 2.0], 1e-3, Window::Hanning), None);
        assert_eq!(spectrum(&[1.0; 10], 0.0, Window::Hanning), None);
        assert!((dbv(1.0)).abs() < 1e-12);
    }
}
