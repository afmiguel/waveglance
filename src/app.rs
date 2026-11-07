//! Oscilloscope-style user interface.

use crate::parser::{self, Acquisition, Waveform};
use eframe::egui::{self, Color32, RichText};
use egui_plot::{HoverPosition, Legend, Line, LineStyle, Plot, PlotPoint, VLine};
use std::ops::RangeInclusive;
use std::sync::mpsc::{Receiver, Sender, channel};

const CH1_COLOR: Color32 = Color32::from_rgb(255, 235, 0); // bright yellow
const CH2_COLOR: Color32 = Color32::from_rgb(0, 230, 255); // bright cyan
const TRIGGER_COLOR: Color32 = Color32::from_rgb(255, 140, 0);
const SCREEN_BG: Color32 = Color32::from_rgb(10, 10, 10);
const GRID_COLOR: Color32 = Color32::from_rgb(90, 90, 90);

/// A file read by the open dialog or by drag-and-drop: (name, bytes or error).
type Loaded = (String, Result<Vec<u8>, String>);

/// Automatic scaling of the plot view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Show the full time span of the waveform. The voltage axis does not change.
    Horizontal,
    /// Scale the voltage axis to the samples between the first and last
    /// instant shown now. The time axis does not change.
    Vertical,
    /// Horizontal fit, then vertical fit.
    Both,
}

/// Application state.
pub struct ScopeApp {
    /// CH1 points `[time_s, volts]`.
    pub channel_1_data: Vec<[f64; 2]>,
    /// CH2 points `[time_s, volts]`.
    pub channel_2_data: Vec<[f64; 2]>,
    /// CH1 volts/division (`None` = channel off).
    pub ch1_v_div: Option<f64>,
    /// CH2 volts/division (`None` = channel off).
    pub ch2_v_div: Option<f64>,
    /// Seconds per horizontal division.
    pub timebase: Option<f64>,

    file_name: Option<String>,
    waveform: Option<Waveform>,
    error: Option<String>,
    /// Changes on each load, so that the plot resets its view.
    load_count: u64,
    /// Fit requested by a button, applied on the next plot frame.
    pending_fit: Option<Fit>,
    /// File reads finish here (they are asynchronous on the web).
    inbox_tx: Sender<Loaded>,
    inbox_rx: Receiver<Loaded>,
}

impl ScopeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        let mut visuals = egui::Visuals::dark();
        visuals.extreme_bg_color = SCREEN_BG; // egui_plot background
        cc.egui_ctx.set_visuals_of(egui::Theme::Dark, visuals);

        let (inbox_tx, inbox_rx) = channel();
        Self {
            channel_1_data: Vec::new(),
            channel_2_data: Vec::new(),
            ch1_v_div: None,
            ch2_v_div: None,
            timebase: None,
            file_name: None,
            waveform: None,
            error: None,
            load_count: 0,
            pending_fit: None,
            inbox_tx,
            inbox_rx,
        }
    }

    /// Parse the bytes of a file and show the result.
    pub fn load_bytes(&mut self, name: String, bytes: &[u8]) {
        match parser::parse(bytes) {
            Ok(w) => {
                self.channel_1_data = w.ch1.as_ref().map(|c| c.points.clone()).unwrap_or_default();
                self.channel_2_data = w.ch2.as_ref().map(|c| c.points.clone()).unwrap_or_default();
                self.ch1_v_div = w.ch1.as_ref().map(|c| c.volts_per_div);
                self.ch2_v_div = w.ch2.as_ref().map(|c| c.volts_per_div);
                self.timebase = Some(w.time_per_div_s);
                for warning in &w.warnings {
                    log::warn!("{name}: {warning}");
                }
                self.waveform = Some(w);
                self.error = None;
                self.load_count += 1;
            }
            Err(e) => self.error = Some(format!("{name}: {e}")),
        }
        self.file_name = Some(name);
    }

    fn receive_loaded_files(&mut self) {
        while let Ok((name, result)) = self.inbox_rx.try_recv() {
            match result {
                Ok(bytes) => self.load_bytes(name, &bytes),
                Err(e) => {
                    self.error = Some(format!("{name}: {e}"));
                    self.file_name = Some(name);
                }
            }
        }
    }

    /// Read files dropped on the window.
    /// Native: read from `file.path()`. Web: the browser gives only bytes.
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        let Some(file) = dropped.into_iter().next() else {
            return;
        };
        let name = file.path().file_name().map_or_else(
            || "dropped file".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        );

        #[cfg(not(target_arch = "wasm32"))]
        {
            let result = std::fs::read(file.path()).map_err(|e| e.to_string());
            let _ = self.inbox_tx.send((name, result));
        }
        #[cfg(target_arch = "wasm32")]
        {
            let tx = self.inbox_tx.clone();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = file.bytes_async().await;
                let _ = tx.send((name, result));
                ctx.request_repaint();
            });
        }
    }

    /// Show the "Open file" dialog (`rfd`).
    fn open_file_dialog(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = ctx;
            let picked = rfd::FileDialog::new()
                .add_filter("DPOX180H waveform", &["wav", "WAV"])
                .pick_file();
            if let Some(path) = picked {
                let name = path
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                let result = std::fs::read(&path).map_err(|e| e.to_string());
                let _ = self.inbox_tx.send((name, result));
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let tx = self.inbox_tx.clone();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let picked = rfd::AsyncFileDialog::new()
                    .add_filter("DPOX180H waveform", &["wav", "WAV"])
                    .pick_file()
                    .await;
                if let Some(file) = picked {
                    let bytes = file.read().await;
                    let _ = tx.send((file.file_name(), Ok(bytes)));
                    ctx.request_repaint();
                }
            });
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("FNIRSI DPOX180H")
                    .strong()
                    .color(Color32::LIGHT_GRAY),
            );
            ui.separator();
            if ui.button("📂 Open file…").clicked() {
                self.open_file_dialog(ui.ctx());
            }
            let loaded = self.waveform.is_some();
            for (label, fit, tip) in [
                ("↔ Fit H", Fit::Horizontal, "Show the full time span"),
                (
                    "↕ Fit V",
                    Fit::Vertical,
                    "Fit the voltage axis to the samples between the first and last instant shown",
                ),
                (
                    "⛶ Fit both",
                    Fit::Both,
                    "Full time span, then fit the voltage axis",
                ),
            ] {
                if ui
                    .add_enabled(loaded, egui::Button::new(label))
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.pending_fit = Some(fit);
                }
            }
            if let Some(name) = &self.file_name {
                ui.label(RichText::new(name).monospace());
            }
            if let Some(err) = &self.error {
                ui.label(RichText::new(format!("⚠ {err}")).color(Color32::from_rgb(255, 90, 90)));
            }
        });
    }

    /// On-screen display: the digital readouts of the scope.
    fn osd(&self, ui: &mut egui::Ui) {
        let probe = |ch: Option<&parser::Channel>| {
            ch.map_or(String::new(), |c| format!(" {}X", c.probe_factor))
        };
        let ch1_probe = probe(self.waveform.as_ref().and_then(|w| w.ch1.as_ref()));
        let ch2_probe = probe(self.waveform.as_ref().and_then(|w| w.ch2.as_ref()));
        ui.horizontal_wrapped(|ui| {
            readout(ui, "CH1", self.ch1_v_div.map(|v| format!("{}/div{ch1_probe}", si(v, "V"))), CH1_COLOR);
            readout(ui, "CH2", self.ch2_v_div.map(|v| format!("{}/div{ch2_probe}", si(v, "V"))), CH2_COLOR);
            readout(ui, "H", self.timebase.map(|t| format!("{}/div", si(t, "s"))), Color32::WHITE);

            let Some(w) = &self.waveform else { return };
            let mode = match w.acquisition {
                Acquisition::RealTime { sample_rate_hz } => format!("{}Sa/s", si_bare(sample_rate_hz)),
                Acquisition::Equivalent { frames } => format!("ETS · frame 1 of {frames}"),
            };
            readout(ui, "MODE", Some(mode), Color32::LIGHT_GRAY);
            readout(ui, "PTS", Some(w.samples_per_channel.to_string()), Color32::LIGHT_GRAY);

            let clipped: Vec<&str> = [("CH1", &w.ch1), ("CH2", &w.ch2)]
                .iter()
                .filter(|(_, c)| c.as_ref().is_some_and(|c| c.clipped))
                .map(|(n, _)| *n)
                .collect();
            if !clipped.is_empty() {
                ui.label(RichText::new(format!("⚠ {} clipped", clipped.join(", "))).color(Color32::from_rgb(255, 90, 90)));
            }
            for warning in &w.warnings {
                ui.label(RichText::new(format!("⚠ {warning}")).color(Color32::from_rgb(255, 180, 0)));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("wheel: zoom · shift: time only · alt: volts only · drag: pan · double-click: reset")
                        .small()
                        .color(Color32::GRAY),
                );
            });
        });
    }

    fn scope_screen(&mut self, ui: &mut egui::Ui) {
        let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        if self.waveform.is_none() || hovering {
            let text = if hovering {
                "Release to load the file"
            } else {
                "Drop a DPOX180H .wav file here, or click \"Open file…\""
            };
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new(text).size(18.0).color(Color32::GRAY))
            });
            return;
        }

        let fit = self.pending_fit.take();
        Plot::new(("scope", self.load_count))
            .show_grid(true)
            .grid_color(GRID_COLOR)
            .legend(Legend::default())
            .x_axis_label("Time (trigger at t = 0)")
            .y_axis_label("Voltage")
            .x_axis_formatter(|mark, _| si(mark.value, "s"))
            .y_axis_formatter(|mark, _| si(mark.value, "V"))
            .label_formatter(|hover: &HoverPosition<'_>| {
                let (name, p): (&str, &PlotPoint) = match hover {
                    HoverPosition::NearDataPoint {
                        plot_name,
                        position,
                        ..
                    } => (plot_name, position),
                    HoverPosition::Elsewhere { position, .. } => ("", position),
                };
                let head = if name.is_empty() {
                    String::new()
                } else {
                    format!("{name}\n")
                };
                Some(format!("{head}t = {}\nv = {}", si(p.x, "s"), si(p.y, "V")))
            })
            // Mouse wheel zooms (like a scope knob); drag pans; double-click resets.
            // Shift + wheel zooms only the time axis, Alt + wheel only the voltage axis.
            .allow_scroll(false)
            .show(ui, |plot| {
                if plot.response().hovered() {
                    let (delta, modifiers) =
                        plot.ctx().input(|i| (i.smooth_scroll_delta, i.modifiers));
                    let amount = delta.x + delta.y;
                    if amount != 0.0 {
                        let f = (amount / 200.0).exp();
                        let factor = if modifiers.shift {
                            egui::vec2(f, 1.0)
                        } else if modifiers.alt {
                            egui::vec2(1.0, f)
                        } else {
                            egui::vec2(f, f)
                        };
                        plot.zoom_bounds_around_hovered(factor);
                    }
                }
                if let Some(fit) = fit {
                    let channels = [
                        self.channel_1_data.as_slice(),
                        self.channel_2_data.as_slice(),
                    ];
                    let mut time = plot.plot_bounds().range_x();
                    if matches!(fit, Fit::Horizontal | Fit::Both)
                        && let Some(full) = time_span(&channels)
                    {
                        time = full.clone();
                        plot.set_plot_bounds_x(full);
                    }
                    if matches!(fit, Fit::Vertical | Fit::Both)
                        && let Some(volts) = voltage_span_in(&channels, &time)
                    {
                        plot.set_plot_bounds_y(volts);
                    }
                }
                plot.vline(
                    VLine::new("Trigger", 0.0)
                        .color(TRIGGER_COLOR)
                        .style(LineStyle::Dashed { length: 6.0 }),
                );
                if !self.channel_1_data.is_empty() {
                    plot.line(
                        Line::new("CH1", self.channel_1_data.clone())
                            .color(CH1_COLOR)
                            .width(1.5),
                    );
                }
                if !self.channel_2_data.is_empty() {
                    plot.line(
                        Line::new("CH2", self.channel_2_data.clone())
                            .color(CH2_COLOR)
                            .width(1.5),
                    );
                }
            });
    }
}

impl eframe::App for ScopeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_dropped_files(&ctx);
        self.receive_loaded_files();

        egui::Panel::top("top_bar").show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("osd").show(ui, |ui| self.osd(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ui.style()).fill(Color32::BLACK))
            .show(ui, |ui| self.scope_screen(ui));
    }
}

/// Time span of all the channels.
pub fn time_span(channels: &[&[[f64; 2]]]) -> Option<RangeInclusive<f64>> {
    let (lo, hi) = channels
        .iter()
        .flat_map(|c| c.iter())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p[0]), hi.max(p[0]))
        });
    (lo < hi).then_some(lo..=hi)
}

/// Voltage range of the samples with a time inside `time`, plus a 5% margin.
/// `None` if no sample is in that window.
pub fn voltage_span_in(
    channels: &[&[[f64; 2]]],
    time: &RangeInclusive<f64>,
) -> Option<RangeInclusive<f64>> {
    let (lo, hi) = channels
        .iter()
        .flat_map(|c| c.iter())
        .filter(|p| time.contains(&p[0]))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p[1]), hi.max(p[1]))
        });
    if lo > hi {
        return None;
    }
    // A flat signal still gets a visible range.
    let margin = if hi > lo {
        (hi - lo) * 0.05
    } else {
        lo.abs().max(1e-3) * 0.1
    };
    Some(lo - margin..=hi + margin)
}

/// One OSD readout: a coloured label and its value.
fn readout(ui: &mut egui::Ui, label: &str, value: Option<String>, color: Color32) {
    let value = value.unwrap_or_else(|| "OFF".to_owned());
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, color))
        .corner_radius(3)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("{label} {value}"))
                    .monospace()
                    .strong()
                    .color(color),
            );
        });
}

/// Format a value with an SI prefix and unit, for example `0.2, "V"` → `200 mV`.
pub fn si(value: f64, unit: &str) -> String {
    format!("{}{unit}", si_bare(value))
}

/// Format a value with an SI prefix and a trailing space, for example `4e8` → `400 M`.
pub fn si_bare(value: f64) -> String {
    const PREFIXES: [(f64, &str); 9] = [
        (1e9, "G"),
        (1e6, "M"),
        (1e3, "k"),
        (1.0, ""),
        (1e-3, "m"),
        (1e-6, "µ"),
        (1e-9, "n"),
        (1e-12, "p"),
        (1e-15, "f"),
    ];
    if value == 0.0 || !value.is_finite() {
        return format!("{value} ");
    }
    let mag = value.abs();
    let (scale, prefix) = PREFIXES
        .iter()
        .find(|(s, _)| mag >= s * 0.9995)
        .copied()
        .unwrap_or((1e-15, "f"));
    let scaled = value / scale;
    let text = format!("{scaled:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text} {prefix}")
}

#[cfg(test)]
mod tests {
    use super::{si, time_span, voltage_span_in};

    #[test]
    fn fit_vertical_uses_only_the_visible_time_window() {
        // CH1: a 10 V spike at t = 5, small values elsewhere. CH2: -1 V at t = 1.
        let ch1 = [[0.0, 0.1], [1.0, 0.2], [2.0, 0.3], [5.0, 10.0]];
        let ch2 = [[1.0, -1.0], [6.0, -5.0]];
        let chans = [&ch1[..], &ch2[..]];
        assert_eq!(time_span(&chans), Some(0.0..=6.0));
        // Window 0..2 excludes the spike and the -5 V sample.
        let r = voltage_span_in(&chans, &(0.0..=2.0)).unwrap();
        let (lo, hi) = (*r.start(), *r.end());
        assert!(
            (lo - (-1.0 - 0.065)).abs() < 1e-12 && (hi - (0.3 + 0.065)).abs() < 1e-12,
            "{r:?}"
        );
        // No samples in the window: no change.
        assert_eq!(voltage_span_in(&chans, &(10.0..=11.0)), None);
        // Flat signal: still a non-empty range.
        let flat = [[0.0, 0.5], [1.0, 0.5]];
        let r = voltage_span_in(&[&flat[..]], &(0.0..=1.0)).unwrap();
        assert!(r.start() < &0.5 && r.end() > &0.5);
    }

    #[test]
    fn si_prefixes() {
        assert_eq!(si(0.2, "V"), "200 mV");
        assert_eq!(si(5e-9, "s"), "5 ns");
        assert_eq!(si(-30e-6, "s"), "-30 µs");
        assert_eq!(si(1.0, "V"), "1 V");
        assert_eq!(si(0.0, "V"), "0 V");
        assert_eq!(si(4e8, "Sa/s"), "400 MSa/s");
        assert_eq!(si(0.0125, "V"), "12.5 mV");
    }
}
