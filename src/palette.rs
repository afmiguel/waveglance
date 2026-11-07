//! Colours of the two screen themes:
//! - dark: black screen, bright traces (like the scope);
//! - light: white screen for printing, with darker traces so that they stay
//!   easy to see on white paper.
//!
//! The theme is global (one window); the app sets it at the start of each
//! frame with [`set_light`].

use eframe::egui::Color32;
use std::sync::atomic::{AtomicBool, Ordering};

static LIGHT: AtomicBool = AtomicBool::new(false);

/// Use the light (white screen) colours.
pub fn set_light(on: bool) {
    LIGHT.store(on, Ordering::Relaxed);
}

pub fn is_light() -> bool {
    LIGHT.load(Ordering::Relaxed)
}

/// The dark-theme colour, or the light-theme colour.
fn pick(dark: Color32, light: Color32) -> Color32 {
    if is_light() { light } else { dark }
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// CH1: bright yellow, or dark yellow (ochre) on white.
pub fn ch1() -> Color32 {
    pick(rgb(255, 235, 0), rgb(165, 120, 0))
}

/// CH2: bright cyan, or dark cyan on white.
pub fn ch2() -> Color32 {
    pick(rgb(0, 230, 255), rgb(0, 115, 165))
}

pub fn trigger() -> Color32 {
    pick(rgb(255, 140, 0), rgb(210, 85, 0))
}

/// Time cursors (vertical lines).
pub fn time_cursor() -> Color32 {
    pick(rgb(255, 110, 255), rgb(160, 0, 160))
}

/// Voltage cursors (horizontal lines).
pub fn volt_cursor() -> Color32 {
    pick(rgb(120, 255, 120), rgb(0, 125, 45))
}

/// Reference waveform of CH1 and CH2: faint, behind the traces.
pub fn reference(channel: usize) -> Color32 {
    match channel {
        0 => pick(rgb(170, 170, 170), rgb(125, 125, 125)),
        _ => pick(rgb(140, 150, 230), rgb(95, 105, 180)),
    }
}

/// Plot background.
pub fn screen_bg() -> Color32 {
    pick(rgb(10, 10, 10), Color32::WHITE)
}

/// Background around the plots.
pub fn panel_bg() -> Color32 {
    pick(Color32::BLACK, rgb(245, 245, 245))
}

pub fn grid() -> Color32 {
    pick(rgb(90, 90, 90), rgb(185, 185, 185))
}

/// Main text (white on black, near black on white).
pub fn fg() -> Color32 {
    pick(Color32::WHITE, rgb(20, 20, 20))
}

/// Secondary readouts.
pub fn fg_dim() -> Color32 {
    pick(Color32::LIGHT_GRAY, rgb(70, 70, 70))
}

/// Hints and small notes.
pub fn hint() -> Color32 {
    pick(rgb(150, 150, 150), rgb(105, 105, 105))
}

/// A channel that is off or hidden.
pub fn off() -> Color32 {
    pick(rgb(110, 110, 110), rgb(160, 160, 160))
}

/// Measurements made unreliable by a clipped signal.
pub fn warn() -> Color32 {
    pick(rgb(255, 150, 60), rgb(195, 85, 0))
}

/// Parser warnings.
pub fn warn_text() -> Color32 {
    pick(rgb(255, 180, 0), rgb(160, 100, 0))
}

pub fn error() -> Color32 {
    pick(rgb(255, 90, 90), rgb(185, 25, 25))
}

/// The part of the overview that the main plot does not show.
pub fn overview_shade() -> Color32 {
    pick(
        Color32::from_black_alpha(170),
        Color32::from_white_alpha(200),
    )
}

/// The edges of the visible window in the overview.
pub fn overview_edge() -> Color32 {
    pick(Color32::WHITE, rgb(50, 50, 50))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative luminance (sRGB), 0 = black, 1 = white.
    fn luminance(c: Color32) -> f64 {
        let f = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn traces_are_readable_in_both_themes() {
        // WCAG 3:1 for graphics (traces, cursor lines, markers).
        for light in [false, true] {
            set_light(light);
            for c in [
                ch1(),
                ch2(),
                trigger(),
                time_cursor(),
                volt_cursor(),
                warn(),
                error(),
            ] {
                let k = contrast(c, screen_bg());
                assert!(k >= 3.0, "light={light}: {c:?} contrast {k:.2}");
            }
            assert!(contrast(fg(), screen_bg()) >= 7.0);
        }
        set_light(false);
    }
}
