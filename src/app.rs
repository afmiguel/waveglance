//! Oscilloscope-style user interface.

use crate::cursors::{self, CursorMode, Cursors};
use crate::measure::{self, Measurement, Unit};
use crate::parser::{self, Acquisition, Waveform};
use eframe::egui::{self, Color32, RichText};
use egui_plot::{HoverPosition, Legend, Line, LineStyle, Plot, PlotPoint, PlotTransform, VLine};
use std::ops::RangeInclusive;
use std::sync::mpsc::{Receiver, Sender, channel};

const CH1_COLOR: Color32 = Color32::from_rgb(255, 235, 0); // bright yellow
const CH2_COLOR: Color32 = Color32::from_rgb(0, 230, 255); // bright cyan
const TRIGGER_COLOR: Color32 = Color32::from_rgb(255, 140, 0);
const SCREEN_BG: Color32 = Color32::from_rgb(10, 10, 10);
const GRID_COLOR: Color32 = Color32::from_rgb(90, 90, 90);
/// Screen points for one wheel "line" (one notch of a mouse wheel on desktop).
/// A browser sends about 100 points per notch.
const WHEEL_LINE_POINTS: f32 = 50.0;
/// Wheel distance (screen points) that zooms by a factor of e.
const WHEEL_ZOOM_POINTS: f32 = 300.0;
/// Storage key of the measurement settings.
const MEASURE_KEY: &str = "measure";
/// A channel whose amplitude is below this many divisions is noise: no
/// edges, period or widths are measured.
const MIN_AMPLITUDE_DIV: f64 = 0.1;

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
    /// Measurement cursors.
    cursors: Cursors,
    /// Selected automatic measurements.
    measure: measure::Settings,
    /// Plot transform of the last frame, to find the cursor under the pointer
    /// before the plot is built.
    last_transform: Option<PlotTransform>,
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

        let measure = cc
            .storage
            .and_then(|s| eframe::get_value(s, MEASURE_KEY))
            .unwrap_or_default();
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
            cursors: Cursors::default(),
            measure,
            last_transform: None,
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
                // New file: cursors at 1/4 and 3/4 of the waveform.
                let channels = [
                    self.channel_1_data.as_slice(),
                    self.channel_2_data.as_slice(),
                ];
                if let Some(time) = time_span(&channels)
                    && let Some(volts) = voltage_span_in(&channels, &time)
                {
                    self.cursors.place_in(&time, &volts);
                }
                self.last_transform = None;
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
        ui.horizontal_wrapped(|ui| {
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
                ("⬌ Fit H", Fit::Horizontal, "Show the full time span"),
                (
                    "⬍ Fit V",
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
            ui.separator();
            ui.label("Cursors:");
            let mut mode = self.cursors.mode;
            for (label, m) in [
                ("Off", CursorMode::Off),
                ("Time", CursorMode::Time),
                ("Volt", CursorMode::Volt),
                ("Both", CursorMode::Both),
            ] {
                ui.add_enabled_ui(loaded, |ui| ui.selectable_value(&mut mode, m, label));
            }
            if mode != self.cursors.mode {
                self.cursors.set_mode(mode);
            }
            ui.separator();
            // Always enabled: the choice can be made before a file is loaded.
            ui.menu_button("📏 Measure", |ui| self.measure_menu(ui))
                .response
                .on_hover_text("Choose the automatic measurements to show");
            ui.separator();
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
            // A normal wrapped label: it goes to the next line on a narrow window
            // instead of overlapping the readouts.
            ui.label(
                RichText::new("wheel: zoom (shift: time, alt: volts) · wheel on an axis: pan · drag: pan · double-click: reset")
                    .small()
                    .color(Color32::GRAY),
            );
        });
    }

    /// Contents of the "Measure" menu.
    fn measure_menu(&mut self, ui: &mut egui::Ui) {
        let m = &mut self.measure;
        ui.horizontal(|ui| {
            ui.label("Channels:");
            ui.checkbox(&mut m.channels[0], RichText::new("CH1").color(CH1_COLOR));
            ui.checkbox(&mut m.channels[1], RichText::new("CH2").color(CH2_COLOR));
        });
        ui.horizontal(|ui| {
            ui.label("Samples:");
            for w in measure::Window::ALL {
                let tip = match w {
                    measure::Window::Screen => "The time span shown now",
                    measure::Window::Record => "All the samples in the file",
                    measure::Window::Cursors => {
                        "Between the time cursors (the screen if they are off)"
                    }
                };
                ui.selectable_value(&mut m.window, w, w.label())
                    .on_hover_text(tip);
            }
        });
        ui.separator();
        for item in Measurement::ALL {
            let mut on = m.is_selected(item);
            if ui
                .checkbox(&mut on, item.label())
                .on_hover_text(item.description())
                .changed()
            {
                m.set_selected(item, on);
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("All").clicked() {
                m.selected = Measurement::ALL.to_vec();
            }
            if ui.button("None").clicked() {
                m.selected.clear();
            }
            if ui.button("Default").clicked() {
                *m = measure::Settings::default();
            }
        });
    }

    /// The time window of the measurements, and its name for the readout.
    fn measure_window(&self) -> Option<(RangeInclusive<f64>, &'static str)> {
        let channels = [
            self.channel_1_data.as_slice(),
            self.channel_2_data.as_slice(),
        ];
        let record = time_span(&channels)?;
        let screen = || {
            self.last_transform.as_ref().map_or_else(
                || (record.clone(), "record"),
                |t| (t.bounds().range_x(), "screen"),
            )
        };
        Some(match self.measure.window {
            measure::Window::Record => (record.clone(), "record"),
            measure::Window::Screen => screen(),
            measure::Window::Cursors if self.cursors.mode.shows_time() => {
                let [a, b] = self.cursors.t;
                (a.min(b)..=a.max(b), "T1–T2")
            }
            measure::Window::Cursors => screen(),
        })
    }

    /// Automatic measurement readouts, one row per channel.
    fn measure_readout(&self, ui: &mut egui::Ui) {
        let Some((window, window_name)) = self.measure_window() else {
            return;
        };
        let Some(w) = &self.waveform else { return };
        for (i, (name, ch, color)) in [("CH1", &w.ch1, CH1_COLOR), ("CH2", &w.ch2, CH2_COLOR)]
            .into_iter()
            .enumerate()
        {
            let Some(ch) = ch else { continue };
            if !self.measure.channels[i] {
                continue;
            }
            let r = measure::measure(&ch.points, &window, MIN_AMPLITUDE_DIV * ch.volts_per_div);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("{name} ({window_name})"))
                        .monospace()
                        .strong()
                        .color(color),
                );
                for m in self.measure.shown() {
                    let value = r.get(m).map_or_else(
                        || "—".to_owned(),
                        |v| match m.unit() {
                            Unit::Volt => si(v, "V"),
                            Unit::Second => si(v, "s"),
                            Unit::Hertz => si(v, "Hz"),
                            Unit::Percent => format!("{v:.1} %"),
                        },
                    );
                    readout(ui, m.label(), Some(value), color).on_hover_text(m.description());
                }
            });
        }
    }

    /// Show the measurement panel?
    fn shows_measurements(&self) -> bool {
        let Some(w) = &self.waveform else {
            return false;
        };
        let on = [w.ch1.is_some(), w.ch2.is_some()];
        !self.measure.selected.is_empty() && (0..2).any(|i| on[i] && self.measure.channels[i])
    }

    /// Cursor readouts: positions, differences and channel values at T1/T2.
    fn cursor_readout(&self, ui: &mut egui::Ui) {
        let c = &self.cursors;
        if c.mode.shows_time() {
            ui.horizontal_wrapped(|ui| {
                let color = cursors::TIME_COLOR;
                readout(ui, "T1", Some(si(c.t[0], "s")), color);
                readout(ui, "T2", Some(si(c.t[1], "s")), color);
                readout(ui, "ΔT", Some(si(c.delta_t(), "s")), color);
                readout(
                    ui,
                    "1/ΔT",
                    Some(c.frequency().map_or("—".to_owned(), |f| si(f, "Hz"))),
                    color,
                );
                for (name, data, color) in [
                    ("CH1", &self.channel_1_data, CH1_COLOR),
                    ("CH2", &self.channel_2_data, CH2_COLOR),
                ] {
                    if data.is_empty() {
                        continue;
                    }
                    let at = |t| cursors::value_at(data, t);
                    let fmt = |v: Option<f64>| v.map_or("—".to_owned(), |v| si(v, "V"));
                    let (a, b) = (at(c.t[0]), at(c.t[1]));
                    let delta = a.zip(b).map(|(a, b)| b - a);
                    let text = format!("@T1 {}  @T2 {}  Δ {}", fmt(a), fmt(b), fmt(delta));
                    readout(ui, name, Some(text), color);
                }
            });
        }
        if c.mode.shows_volt() {
            ui.horizontal_wrapped(|ui| {
                let color = cursors::VOLT_COLOR;
                readout(ui, "V1", Some(si(c.v[0], "V")), color);
                readout(ui, "V2", Some(si(c.v[1], "V")), color);
                readout(ui, "ΔV", Some(si(c.delta_v(), "V")), color);
            });
        }
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
        // Near a cursor line, a drag moves the cursor instead of the plot.
        let allow_pan = self
            .cursors
            .handle_pointer(ui.ctx(), self.last_transform.as_ref());
        // The plot and its axis labels fill this area.
        let outer = ui.available_rect_before_wrap();
        let response = Plot::new(("scope", self.load_count))
            .allow_drag(allow_pan)
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
            // Mouse wheel over the plot zooms (like a scope knob); drag pans;
            // double-click resets. Shift + wheel zooms only the time axis,
            // Alt + wheel only the voltage axis. Wheel over an axis pans that axis.
            .allow_scroll(false)
            .show(ui, |plot| {
                let (delta, modifiers, pointer) = plot
                    .ctx()
                    .input(|i| (wheel_delta(i), i.modifiers, i.pointer.latest_pos()));
                let amount = delta.x + delta.y;
                let frame = *plot.transform().frame();
                let zone = pointer.and_then(|p| axis_zone(p, frame, outer));
                // An axis label area pans that axis; inside the plot frame the wheel zooms.
                // (The plot response also covers the axis areas, so test the zones first.)
                if amount != 0.0 {
                    if let Some(axis) = zone {
                        // Pan by the wheel distance in screen points.
                        // Wheel up: earlier times (left) / higher voltages (up).
                        let [dx, dy] = plot.transform().dvalue_dpos();
                        let shift = match axis {
                            Axis::X => egui::vec2((-f64::from(amount) * dx) as f32, 0.0),
                            Axis::Y => egui::vec2(0.0, (-f64::from(amount) * dy) as f32),
                        };
                        plot.translate_bounds(shift);
                    } else if pointer.is_some_and(|p| frame.contains(p)) {
                        let f = (amount / WHEEL_ZOOM_POINTS).exp();
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
                if zone.is_some() {
                    plot.ctx().set_cursor_icon(match zone {
                        Some(Axis::X) => egui::CursorIcon::ResizeHorizontal,
                        _ => egui::CursorIcon::ResizeVertical,
                    });
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
                self.cursors.place_pending(&plot.plot_bounds());
                self.cursors.update_drag(plot);
                self.cursors.draw(plot);
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
        self.cursors
            .update_pointer_icon(ui.ctx(), &response.transform);
        // The measurements use the view of the last frame: draw once more
        // when the view changes, so that they are never one frame late.
        let old_x = self.last_transform.as_ref().map(|t| t.bounds().range_x());
        if old_x != Some(response.transform.bounds().range_x()) {
            ui.ctx().request_repaint();
        }
        self.last_transform = Some(response.transform);
    }
}

impl eframe::App for ScopeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_dropped_files(&ctx);
        self.receive_loaded_files();

        egui::Panel::top("top_bar").show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("osd").show(ui, |ui| self.osd(ui));
        if self.shows_measurements() {
            egui::Panel::bottom("measurements").show(ui, |ui| self.measure_readout(ui));
        }
        if self.waveform.is_some() && self.cursors.mode != CursorMode::Off {
            egui::Panel::bottom("cursor_readout").show(ui, |ui| self.cursor_readout(ui));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ui.style()).fill(Color32::BLACK))
            .show(ui, |ui| self.scope_screen(ui));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, MEASURE_KEY, &self.measure);
    }
}

/// Mouse-wheel movement in this frame, in screen points, from the raw wheel
/// events. Unlike `smooth_scroll_delta`, nothing carries over to the next
/// frames, so a zoom or pan stops as soon as the wheel stops and never
/// reaches another area that the pointer moves to.
fn wheel_delta(input: &egui::InputState) -> egui::Vec2 {
    input
        .events
        .iter()
        .filter_map(|e| match e {
            egui::Event::MouseWheel { unit, delta, .. } => Some(match unit {
                egui::MouseWheelUnit::Point => *delta,
                egui::MouseWheelUnit::Line => *delta * WHEEL_LINE_POINTS,
                egui::MouseWheelUnit::Page => *delta * input.viewport_rect().height(),
            }),
            _ => None,
        })
        .fold(egui::Vec2::ZERO, |acc, d| acc + d)
}

/// A plot axis, for the axis label areas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

/// The axis label area that contains `pos`: below the plot frame (time axis)
/// or left of it (voltage axis), inside `outer`, the area of the whole plot.
pub fn axis_zone(pos: egui::Pos2, frame: egui::Rect, outer: egui::Rect) -> Option<Axis> {
    if !outer.contains(pos) {
        return None;
    }
    if pos.y > frame.bottom() && pos.x >= frame.left() && pos.x <= frame.right() {
        Some(Axis::X)
    } else if pos.x < frame.left() && pos.y >= frame.top() && pos.y <= frame.bottom() {
        Some(Axis::Y)
    } else {
        None
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
fn readout(
    ui: &mut egui::Ui,
    label: &str,
    value: Option<String>,
    color: Color32,
) -> egui::Response {
    let value = value.unwrap_or_else(|| "OFF".to_owned());
    // One widget (not a frame around a label), so that a wrapped row moves the
    // whole box to the next line.
    let text = RichText::new(format!("{label} {value}"))
        .monospace()
        .strong()
        .color(color);
    ui.add(
        egui::Button::new(text)
            .fill(Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.0, color))
            .corner_radius(3)
            .sense(egui::Sense::hover()),
    )
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
    use super::{Axis, axis_zone, si, time_span, voltage_span_in};
    use eframe::egui::{Rect, pos2};

    #[test]
    fn axis_zones() {
        let outer = Rect::from_min_max(pos2(0.0, 0.0), pos2(500.0, 400.0));
        let frame = Rect::from_min_max(pos2(60.0, 10.0), pos2(490.0, 360.0));
        assert_eq!(axis_zone(pos2(200.0, 380.0), frame, outer), Some(Axis::X));
        assert_eq!(axis_zone(pos2(30.0, 200.0), frame, outer), Some(Axis::Y));
        assert_eq!(
            axis_zone(pos2(200.0, 200.0), frame, outer),
            None,
            "inside the plot"
        );
        assert_eq!(axis_zone(pos2(30.0, 380.0), frame, outer), None, "corner");
        assert_eq!(axis_zone(pos2(200.0, 450.0), frame, outer), None, "outside");
    }

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
