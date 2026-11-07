//! Oscilloscope-style user interface.

use crate::cursors::{self, CursorMode, Cursors};
use crate::measure::{self, Measurement, Unit};
use crate::parser::{self, Acquisition, Waveform};
use eframe::egui::{self, Color32, RichText};
use egui_plot::{HoverPosition, Line, LineStyle, Plot, PlotPoint, PlotTransform, VLine};
use std::ops::RangeInclusive;
use std::sync::mpsc::{Receiver, Sender, channel};

const CH1_COLOR: Color32 = Color32::from_rgb(255, 235, 0); // bright yellow
const CH2_COLOR: Color32 = Color32::from_rgb(0, 230, 255); // bright cyan
const TRIGGER_COLOR: Color32 = Color32::from_rgb(255, 140, 0);
const SCREEN_BG: Color32 = Color32::from_rgb(10, 10, 10);
const GRID_COLOR: Color32 = Color32::from_rgb(90, 90, 90);
/// Readouts of a channel that is off, and other inactive text.
const OFF_COLOR: Color32 = Color32::from_rgb(110, 110, 110);
/// Measurements that a clipped signal makes unreliable.
const WARN_COLOR: Color32 = Color32::from_rgb(255, 150, 60);
const ERROR_COLOR: Color32 = Color32::from_rgb(255, 90, 90);
/// Below this width (points), the top bar uses compact menus.
const NARROW_WIDTH: f32 = 600.0;
/// Significant digits of the measurement and cursor readouts.
const READOUT_DIGITS: i32 = 4;
/// Screen points for one wheel "line" (one notch of a mouse wheel on desktop).
/// A browser sends about 100 points per notch.
const WHEEL_LINE_POINTS: f32 = 50.0;
/// Wheel distance (screen points) that zooms by a factor of e.
const WHEEL_ZOOM_POINTS: f32 = 300.0;
/// Version of the app, from Cargo.toml.
const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Short git commit of the build (from build.rs).
const GIT_HASH: &str = env!("GIT_HASH");
/// Storage key of the measurement settings.
const MEASURE_KEY: &str = "measure";
/// Storage key of "do not show the splash screen at startup".
const HIDE_SPLASH_KEY: &str = "hide_splash";
/// Storage key of the vertical axis mode. ("y_axis" held the old default,
/// Divisions, saved automatically; a new key starts everyone in Volts.)
const Y_AXIS_KEY: &str = "y_axis_mode";
/// Vertical divisions on the scope screen (±4 around the centre).
const SCREEN_DIVS_Y: f64 = 8.0;
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

/// Vertical axis of the plot.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum YAxis {
    /// Divisions, like a scope screen: each channel uses its own V/div and
    /// ground position, so a small signal is not flat next to a large one.
    Divisions,
    /// Volts at the probe tip; both channels share the axis.
    #[default]
    Volts,
}

/// Vertical mapping of one channel: plot y = volts × `scale` + `offset`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YMap {
    pub scale: f64,
    pub offset: f64,
}

impl YMap {
    pub const IDENTITY: Self = Self {
        scale: 1.0,
        offset: 0.0,
    };

    /// The mapping of a channel in Divisions mode: volts / (V/div), shifted
    /// so that 0 V is at the ground position of the channel.
    pub fn divisions(ch: &parser::Channel) -> Self {
        Self {
            scale: 1.0 / ch.volts_per_div,
            offset: ch.ground_div(),
        }
    }

    pub fn to_plot(self, volts: f64) -> f64 {
        volts * self.scale + self.offset
    }

    pub fn to_volts(self, y: f64) -> f64 {
        (y - self.offset) / self.scale
    }
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
    /// Vertical axis: divisions (per channel) or volts.
    y_axis: YAxis,
    /// The splash screen (welcome and controls) is open.
    splash_open: bool,
    /// Do not open the splash screen at startup (saved).
    hide_splash: bool,
    /// The Help window is open.
    help_open: bool,
    /// CH1 and CH2 shown on the screen (a click on the OSD readout hides or
    /// shows a channel, like the channel button of a scope).
    visible: [bool; 2],
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
        let y_axis = cc
            .storage
            .and_then(|s| eframe::get_value(s, Y_AXIS_KEY))
            .unwrap_or_default();
        let hide_splash: bool = cc
            .storage
            .and_then(|s| eframe::get_value(s, HIDE_SPLASH_KEY))
            .unwrap_or(false);
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
            y_axis,
            splash_open: !hide_splash,
            hide_splash,
            help_open: false,
            visible: [true, true],
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
                let [d1, d2] = self.plot_data();
                let channels = [d1.as_slice(), d2.as_slice()];
                if let Some(time) = time_span(&channels)
                    && let Some(span) = voltage_span_in(&channels, &time)
                {
                    self.cursors.place_in(&time, &span);
                }
                self.last_transform = None;
            }
            Err(e) => {
                self.report_open_error(&name, &e.to_string());
                return;
            }
        }
        self.file_name = Some(name);
    }

    /// Vertical mapping of each channel for the current Y axis mode
    /// (`None` if the channel is off or hidden).
    pub fn y_maps(&self) -> [Option<YMap>; 2] {
        let Some(w) = &self.waveform else {
            return [None, None];
        };
        let map = |ch: &Option<parser::Channel>, i: usize| {
            ch.as_ref()
                .filter(|_| self.visible[i])
                .map(|c| match self.y_axis {
                    YAxis::Divisions => YMap::divisions(c),
                    YAxis::Volts => YMap::IDENTITY,
                })
        };
        [map(&w.ch1, 0), map(&w.ch2, 1)]
    }

    /// The channel points in plot units: `[time_s, y]`.
    fn plot_data(&self) -> [Vec<[f64; 2]>; 2] {
        let maps = self.y_maps();
        let data = [&self.channel_1_data, &self.channel_2_data];
        std::array::from_fn(|i| match maps[i] {
            Some(m) => data[i].iter().map(|p| [p[0], m.to_plot(p[1])]).collect(),
            None => Vec::new(),
        })
    }

    /// Change the Y axis mode. The voltage cursors keep their voltage (on the
    /// first channel that is on). The view resets (the plot id changes).
    fn set_y_axis(&mut self, y_axis: YAxis) {
        let old = self.y_maps().into_iter().flatten().next();
        self.y_axis = y_axis;
        let new = self.y_maps().into_iter().flatten().next();
        if let (Some(old), Some(new)) = (old, new) {
            for v in &mut self.cursors.v {
                *v = new.to_plot(old.to_volts(*v));
            }
        }
        self.last_transform = None;
    }

    /// The Y axis selector: Volts (default) / Div.
    fn y_axis_buttons(&mut self, ui: &mut egui::Ui) {
        let mut y_axis = self.y_axis;
        ui.selectable_value(&mut y_axis, YAxis::Volts, "Volts")
            .on_hover_text("Vertical axis in volts: both channels share the axis");
        ui.selectable_value(&mut y_axis, YAxis::Divisions, "Div").on_hover_text(
            "Vertical axis in divisions, like the scope screen: each channel has its own V/div and ground marker",
        );
        if y_axis != self.y_axis {
            self.set_y_axis(y_axis);
            ui.close();
        }
    }

    /// A file could not be opened. The file shown before (if any) stays, and
    /// the message says so, so that its data is not taken for the new file.
    fn report_open_error(&mut self, name: &str, error: &str) {
        let still = match &self.file_name {
            Some(shown) if self.waveform.is_some() => format!(" Still showing {shown}."),
            _ => String::new(),
        };
        self.error = Some(format!("Could not open {name}: {error}.{still}"));
    }

    /// The splash screen: what the app does, its version, and the controls.
    /// "Don't show this at startup" is saved; the Help button has the full
    /// instructions.
    fn splash(&mut self, ctx: &egui::Context) {
        if !self.splash_open {
            return;
        }
        let mut open_file = false;
        let modal = egui::Modal::new(egui::Id::unique("splash")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.heading("FNIRSI Viewer");
            ui.label(
                RichText::new(format!("Version {VERSION} · commit {GIT_HASH}"))
                    .small()
                    .color(Color32::GRAY),
            );
            ui.add_space(6.0);
            ui.label(
                "Opens the waveform files (.wav) that FNIRSI DPOX180H, 1013D and 1014D \
                 oscilloscopes save. The files are read on this device: nothing is uploaded.",
            );
            ui.add_space(8.0);
            ui.label(RichText::new("Controls").strong());
            egui::Grid::new("splash_controls")
                .num_columns(2)
                .spacing([16.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    for (what, does) in [
                        ("Mouse wheel", "Zoom (Shift: time only, Alt: volts only)"),
                        ("Wheel on an axis", "Pan along that axis"),
                        ("Drag", "Pan"),
                        ("Double-click", "Reset the view"),
                        ("Drag a dashed line", "Move a cursor (⌖ Cursors menu)"),
                        ("Click CH1 / CH2 readout", "Hide or show the channel"),
                        ("Touch", "Pinch: zoom · drag: pan · double-tap: reset"),
                        ("Esc", "Close this screen or an error message"),
                    ] {
                        ui.label(RichText::new(what).monospace());
                        ui.label(does);
                        ui.end_row();
                    }
                });
            ui.add_space(8.0);
            ui.checkbox(&mut self.hide_splash, "Don't show this at startup");
            ui.label(
                RichText::new("The Help button shows these controls and more.")
                    .small()
                    .color(Color32::GRAY),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("📂 Open file…").clicked() {
                    open_file = true;
                }
                if ui.button("❓ Help").clicked() {
                    self.splash_open = false;
                    self.help_open = true;
                }
                if ui.button("Close").clicked() {
                    self.splash_open = false;
                }
            });
        });
        if modal.should_close() {
            self.splash_open = false;
        }
        if open_file {
            self.splash_open = false;
            self.open_file_dialog(ctx);
        }
    }

    /// The Help window: the main instructions, in short sections.
    fn help(&mut self, ctx: &egui::Context) {
        let mut open = self.help_open;
        egui::Window::new("Help")
            .open(&mut open)
            .default_width(480.0)
            .default_height(ctx.content_rect().height() * 0.7)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, help_text);
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }
        self.help_open = open;
    }

    /// The error banner below the top bar, with a button to close it.
    fn error_banner(&mut self, ui: &mut egui::Ui) {
        let Some(err) = &self.error else { return };
        let mut close = false;
        ui.horizontal(|ui| {
            if ui.button("✕").on_hover_text("Close this message").clicked() {
                close = true;
            }
            ui.add(egui::Label::new(RichText::new(format!("⚠ {err}")).color(ERROR_COLOR)).wrap());
        });
        if close || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.error = None;
        }
    }

    fn receive_loaded_files(&mut self) {
        while let Ok((name, result)) = self.inbox_rx.try_recv() {
            match result {
                Ok(bytes) => self.load_bytes(name, &bytes),
                Err(e) => self.report_open_error(&name, &e),
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
                .add_filter("FNIRSI waveform", &["wav", "WAV"])
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
                    .add_filter("FNIRSI waveform", &["wav", "WAV"])
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
            // The model comes from the loaded file.
            let title = match &self.waveform {
                Some(w) => format!("FNIRSI {}", w.model.name()),
                None => "FNIRSI Viewer".to_owned(),
            };
            ui.label(RichText::new(title).strong().color(Color32::LIGHT_GRAY))
                .on_hover_text("Supported: DPOX180H, 1013D and 1014D (original firmware)");
            // Version, discreet: identifies the build (also between releases).
            ui.label(
                RichText::new(format!("v{VERSION}"))
                    .small()
                    .color(Color32::from_rgb(130, 130, 130)),
            )
            .on_hover_text(format!("fnirsi-viewer {VERSION}, commit {GIT_HASH}"));
            ui.add_space(8.0);
            if ui
                .button("📂 Open file…")
                .on_hover_text("Open a FNIRSI .wav file (or drop it on the window)")
                .clicked()
            {
                self.open_file_dialog(ui.ctx());
            }
            let loaded = self.waveform.is_some();
            // Narrow window (phone): Fit and Cursors go into menus.
            let narrow = ui.ctx().content_rect().width() < NARROW_WIDTH;
            if narrow {
                ui.menu_button("View ▾", |ui| {
                    ui.add_enabled_ui(loaded, |ui| self.fit_buttons(ui));
                    ui.separator();
                    ui.label("Y axis:");
                    ui.horizontal(|ui| self.y_axis_buttons(ui));
                });
            } else {
                ui.add_space(8.0);
                // `horizontal`: inside a wrapped row, a scope Ui is vertical.
                ui.add_enabled_ui(loaded, |ui| ui.horizontal(|ui| self.fit_buttons(ui)));
                ui.add_space(8.0);
                ui.label("Y:");
                self.y_axis_buttons(ui);
            }
            ui.add_space(8.0);
            // Cursor menu: the button shows the mode in use.
            ui.add_enabled_ui(loaded, |ui| {
                let label = match self.cursors.mode {
                    CursorMode::Off => "⌖ Cursors ▾".to_owned(),
                    m => format!("⌖ Cursors: {} ▾", cursor_mode_name(m)),
                };
                ui.menu_button(label, |ui| self.cursor_menu(ui))
                    .response
                    .on_hover_text("Measurement cursors");
            });
            ui.add_space(8.0);
            // Always enabled: the choice can be made before a file is loaded.
            // Stay open while items are ticked; close on a click outside the menu.
            let config = egui::containers::menu::MenuConfig::new()
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
            let (button, _) = egui::containers::menu::MenuButton::new("📏 Measure")
                .config(config)
                .ui(ui, |ui| self.measure_menu(ui));
            button.on_hover_text("Choose the automatic measurements to show");
            ui.add_space(8.0);
            if ui
                .button("❓ Help")
                .on_hover_text("How to use the viewer: controls, cursors, measurements")
                .clicked()
            {
                self.help_open = !self.help_open;
            }
            ui.add_space(8.0);
            if let Some(name) = &self.file_name {
                ui.label(RichText::new(name).monospace());
            }
        });
    }

    /// The three Fit buttons.
    fn fit_buttons(&mut self, ui: &mut egui::Ui) {
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
            if ui.button(label).on_hover_text(tip).clicked() {
                self.pending_fit = Some(fit);
                ui.close();
            }
        }
    }

    /// Contents of the "Cursors" menu: the mode (radio buttons) and an
    /// action to bring the cursors back into view.
    fn cursor_menu(&mut self, ui: &mut egui::Ui) {
        let mut mode = self.cursors.mode;
        for (m, tip) in [
            (CursorMode::Off, "No cursors"),
            (CursorMode::Time, "Time cursors T1 and T2 (vertical lines)"),
            (
                CursorMode::Volt,
                "Voltage cursors V1 and V2 (horizontal lines)",
            ),
            (CursorMode::Both, "Time and voltage cursors"),
        ] {
            ui.radio_value(&mut mode, m, cursor_mode_name(m))
                .on_hover_text(tip);
        }
        if mode != self.cursors.mode {
            self.cursors.set_mode(mode);
            ui.close();
        }
        ui.separator();
        if ui
            .add_enabled(mode != CursorMode::Off, egui::Button::new("Bring to view"))
            .on_hover_text("Move the cursors to 1/4 and 3/4 of the screen")
            .clicked()
        {
            self.cursors.bring_to_view();
            ui.close();
        }
        ui.label(
            RichText::new("Drag a dashed cursor line to move it.")
                .small()
                .color(Color32::GRAY),
        );
    }

    /// On-screen display: the digital readouts of the scope.
    fn osd(&mut self, ui: &mut egui::Ui) {
        // Probe factor and, if the file stores it, the input coupling.
        let probe = |ch: Option<&parser::Channel>| {
            ch.map_or(String::new(), |c| {
                let coupling = c
                    .coupling
                    .map_or(String::new(), |k| format!(" {}", k.name()));
                format!(" {}X{coupling}", c.probe_factor)
            })
        };
        let ch1_probe = probe(self.waveform.as_ref().and_then(|w| w.ch1.as_ref()));
        let ch2_probe = probe(self.waveform.as_ref().and_then(|w| w.ch2.as_ref()));
        ui.horizontal_wrapped(|ui| {
            // Channel readouts are buttons: a click hides or shows the trace.
            for (i, (name, vdiv, probe, color)) in [
                ("CH1", self.ch1_v_div, &ch1_probe, CH1_COLOR),
                ("CH2", self.ch2_v_div, &ch2_probe, CH2_COLOR),
            ]
            .into_iter()
            .enumerate()
            {
                let Some(vdiv) = vdiv else {
                    readout(ui, name, None, color);
                    continue;
                };
                let text = format!("{}/div{probe}", si(vdiv, "V"));
                if channel_button(ui, name, &text, color, self.visible[i]).clicked() {
                    self.visible[i] = !self.visible[i];
                }
            }
            readout(ui, "H", self.timebase.map(|t| format!("{}/div", si(t, "s"))), Color32::WHITE);

            let Some(w) = &self.waveform else { return };
            let mode = match w.acquisition {
                Acquisition::RealTime { sample_rate_hz } => format!("{}Sa/s", si_bare(sample_rate_hz)),
                Acquisition::Equivalent { frames } => format!("ETS · frame 1 of {frames}"),
            };
            readout(ui, "MODE", Some(mode), Color32::LIGHT_GRAY);
            if let Some(t) = &w.trigger {
                let edge = if t.rising { "↑" } else { "↓" };
                readout(
                    ui,
                    "TRIG",
                    Some(format!("{} {edge} CH{}", t.mode, t.source)),
                    TRIGGER_COLOR,
                );
            }
            readout(ui, "PTS", Some(w.samples_per_channel.to_string()), Color32::LIGHT_GRAY);

            let clipped: Vec<&str> = [("CH1", &w.ch1), ("CH2", &w.ch2)]
                .iter()
                .filter(|(_, c)| c.as_ref().is_some_and(|c| c.clipped))
                .map(|(n, _)| *n)
                .collect();
            if !clipped.is_empty() {
                ui.label(RichText::new(format!("⚠ {} clipped", clipped.join(", "))).color(ERROR_COLOR))
                    .on_hover_text("The signal reaches the limit of the input range: amplitude measurements are marked with \"?\"");
            }
            for warning in &w.warnings {
                ui.label(RichText::new(format!("⚠ {warning}")).color(Color32::from_rgb(255, 180, 0)));
            }
            // A normal wrapped label: it goes to the next line on a narrow window
            // instead of overlapping the readouts.
        });
    }

    /// Contents of the "Measure" menu.
    fn measure_menu(&mut self, ui: &mut egui::Ui) {
        // A channel that is off in the loaded file cannot be measured.
        let present = self
            .waveform
            .as_ref()
            .map_or([true, true], |w| [w.ch1.is_some(), w.ch2.is_some()]);
        let m = &mut self.measure;
        ui.horizontal(|ui| {
            ui.label("Channels:");
            for (i, (name, color)) in [("CH1", CH1_COLOR), ("CH2", CH2_COLOR)]
                .into_iter()
                .enumerate()
            {
                ui.add_enabled(
                    present[i],
                    egui::Checkbox::new(&mut m.channels[i], RichText::new(name).color(color)),
                )
                .on_disabled_hover_text(format!("{name} is off in this file"));
            }
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
            if ui
                .button("Default")
                .on_hover_text("Freq and Vpp, both channels, screen samples")
                .clicked()
            {
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
            if !self.measure.channels[i] || !self.visible[i] {
                continue;
            }
            let r = measure::measure(&ch.points, &window, MIN_AMPLITUDE_DIV * ch.volts_per_div);
            let clipped = ch.clipped_in(&window);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("{name} ({window_name})"))
                        .monospace()
                        .strong()
                        .color(color),
                );
                for m in self.measure.shown() {
                    let value = format_measurement(m, r.get(m), ch);
                    // A clipped signal hides its real peaks: voltage values
                    // are not reliable (the real Vpp is larger).
                    if clipped && m.unit() == Unit::Volt {
                        readout(ui, m.label(), Some(format!("?{value}")), WARN_COLOR).on_hover_text(
                            format!("{}\n⚠ {name} is clipped in this window: the real value can be larger", m.description()),
                        );
                    } else {
                        readout(ui, m.label(), Some(value), color).on_hover_text(m.description());
                    }
                }
            });
        }
    }

    /// Show the measurement panel?
    fn shows_measurements(&self) -> bool {
        let Some(w) = &self.waveform else {
            return false;
        };
        let on = [
            w.ch1.is_some() && self.visible[0],
            w.ch2.is_some() && self.visible[1],
        ];
        !self.measure.selected.is_empty() && (0..2).any(|i| on[i] && self.measure.channels[i])
    }

    /// Cursor readouts: positions, differences and channel values at T1/T2.
    fn cursor_readout(&self, ui: &mut egui::Ui) {
        let c = &self.cursors;
        if c.mode.shows_time() {
            ui.horizontal_wrapped(|ui| {
                let color = cursors::TIME_COLOR;
                let fmt_t = |t| fmt_sig(t, "s", READOUT_DIGITS, 0.0);
                readout(ui, "T1", Some(fmt_t(c.t[0])), color);
                readout(ui, "T2", Some(fmt_t(c.t[1])), color);
                readout(ui, "ΔT", Some(fmt_t(c.delta_t())), color);
                readout(
                    ui,
                    "1/ΔT",
                    Some(
                        c.frequency()
                            .map_or("—".to_owned(), |f| fmt_sig(f, "Hz", READOUT_DIGITS, 0.0)),
                    ),
                    color,
                );
                let Some(w) = &self.waveform else { return };
                for (i, (name, ch, color)) in
                    [("CH1", &w.ch1, CH1_COLOR), ("CH2", &w.ch2, CH2_COLOR)]
                        .into_iter()
                        .enumerate()
                {
                    let Some(ch) = ch.as_ref().filter(|_| self.visible[i]) else {
                        continue;
                    };
                    let at = |t| cursors::value_at(&ch.points, t);
                    let lsb = ch.lsb_volts();
                    let fmt = |v: Option<f64>| {
                        v.map_or("—".to_owned(), |v| fmt_sig(v, "V", READOUT_DIGITS, lsb))
                    };
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
                let fmt_v = |v| fmt_sig(v, "V", READOUT_DIGITS, 0.0);
                if self.y_axis == YAxis::Volts {
                    readout(ui, "V1", Some(fmt_v(c.v[0])), color);
                    readout(ui, "V2", Some(fmt_v(c.v[1])), color);
                    readout(ui, "ΔV", Some(fmt_v(c.delta_v())), color);
                    return;
                }
                // Divisions: the cursor lines are in divisions; each channel
                // reads them in its own volts.
                ui.label(RichText::new("V1/V2").monospace().strong().color(color));
                let maps = self.y_maps();
                for (i, (name, ch_color)) in [("CH1", CH1_COLOR), ("CH2", CH2_COLOR)]
                    .into_iter()
                    .enumerate()
                {
                    let Some(m) = maps[i] else { continue };
                    let (a, b) = (m.to_volts(c.v[0]), m.to_volts(c.v[1]));
                    readout(
                        ui,
                        name,
                        Some(format!(
                            "V1 {}  V2 {}  ΔV {}",
                            fmt_v(a),
                            fmt_v(b),
                            fmt_v(b - a)
                        )),
                        ch_color,
                    );
                }
            });
        }
    }

    fn scope_screen(&mut self, ui: &mut egui::Ui) {
        let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        if self.waveform.is_none() || hovering {
            let text = if hovering {
                "Release to load the file"
            } else {
                "Drop a FNIRSI .wav file here (DPOX180H, 1013D, 1014D), or click \"Open file…\""
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
        let maps = self.y_maps();
        let [data_1, data_2] = self.plot_data();
        let divisions = self.y_axis == YAxis::Divisions;
        let mut plot = Plot::new(("scope", self.load_count, self.y_axis));
        plot = if divisions {
            // Like a scope screen: 8 divisions, a line per division, and
            // thinner lines at 1/5 division when zoomed in.
            plot.default_y_bounds(-SCREEN_DIVS_Y / 2.0, SCREEN_DIVS_Y / 2.0)
                .y_axis_label("Divisions")
                .y_grid_spacer(egui_plot::uniform_grid_spacer(|input| {
                    let s = input.base_step_size;
                    if s <= 0.2 {
                        [0.2, 1.0, 4.0]
                    } else if s <= 1.0 {
                        [1.0, 4.0, 20.0]
                    } else {
                        [5.0, 20.0, 100.0]
                    }
                }))
                .y_axis_formatter(|mark, _| {
                    let v = mark.value;
                    if (v - v.round()).abs() < 1e-6 {
                        format!("{}", v.round() as i64)
                    } else {
                        String::new()
                    }
                })
        } else {
            plot.y_axis_label("Voltage")
                .y_axis_formatter(|mark, _| si(mark.value, "V"))
        };
        let response = plot
            .allow_drag(allow_pan)
            .show_grid(true)
            .grid_color(GRID_COLOR)
            // No legend: the OSD names the channels by colour, and a legend
            // covers the trace.
            .x_axis_label("Time (trigger at t = 0)")
            .x_axis_formatter(|mark, _| si(mark.value, "s"))
            .label_formatter(move |hover: &HoverPosition<'_>| {
                let t = |p: &PlotPoint| format!("t = {}", si(p.x, "s"));
                Some(match hover {
                    // On a trace: the value of that channel, in its volts.
                    HoverPosition::NearDataPoint {
                        plot_name,
                        position,
                        ..
                    } => {
                        let map = match *plot_name {
                            "CH1" => maps[0],
                            "CH2" => maps[1],
                            _ => None,
                        };
                        match map {
                            Some(m) => format!(
                                "{plot_name}\n{}\nv = {}",
                                t(position),
                                si(m.to_volts(position.y), "V")
                            ),
                            None => format!("{plot_name}\n{}", t(position)),
                        }
                    }
                    // Elsewhere: the pointer position, not a signal value.
                    HoverPosition::Elsewhere { position, .. } => {
                        if divisions {
                            format!("{}\n{:.2} div", t(position), position.y)
                        } else {
                            format!("{}\npointer {}", t(position), si(position.y, "V"))
                        }
                    }
                })
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
                    let channels = [data_1.as_slice(), data_2.as_slice()];
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
                // A faint line at the trigger time; the marker on the top edge
                // (drawn after the plot) is the main sign. No name: no legend
                // entry and no hover label.
                plot.vline(
                    VLine::new("", 0.0)
                        .color(TRIGGER_COLOR.gamma_multiply(0.35))
                        .width(1.0)
                        .style(LineStyle::Dotted { spacing: 6.0 }),
                );
                if !data_1.is_empty() {
                    plot.line(Line::new("CH1", data_1).color(CH1_COLOR).width(1.5));
                }
                if !data_2.is_empty() {
                    plot.line(Line::new("CH2", data_2).color(CH2_COLOR).width(1.5));
                }
            });
        self.cursors
            .update_pointer_icon(ui.ctx(), &response.transform);
        draw_trigger_marker(ui, &response.transform);
        draw_ground_markers(ui, &response.transform, &maps);
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
        // A file was dropped or opened: the splash screen is no longer needed.
        if self.waveform.is_some() && self.load_count == 1 && self.splash_open {
            self.splash_open = false;
        }
        self.splash(&ctx);
        if self.help_open {
            self.help(&ctx);
        }

        egui::Panel::top("top_bar").show(ui, |ui| self.top_bar(ui));
        if self.error.is_some() {
            egui::Panel::top("error_banner").show(ui, |ui| self.error_banner(ui));
        }
        // No OSD without a file: "CH1 OFF CH2 OFF" would look like a scope
        // that is switched off.
        if self.waveform.is_some() {
            egui::Panel::bottom("osd").show(ui, |ui| self.osd(ui));
        }
        if self.shows_measurements() {
            // Not resizable, no scroll area: the panel follows its content.
            // (A scroll area inside keeps the height of the last frame and
            // hid the CH2 row.)
            egui::Panel::bottom("measurements")
                .resizable(false)
                .show(ui, |ui| self.measure_readout(ui));
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
        eframe::set_value(storage, Y_AXIS_KEY, &self.y_axis);
        eframe::set_value(storage, HIDE_SPLASH_KEY, &self.hide_splash);
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
    // A channel that is off: grey text, no box (like the dimmed label of a
    // scope channel that is off).
    let (value, color, stroke) = match value {
        Some(v) => (v, color, egui::Stroke::new(1.0, color)),
        None => ("OFF".to_owned(), OFF_COLOR, egui::Stroke::NONE),
    };
    // One widget (not a frame around a label), so that a wrapped row moves the
    // whole box to the next line.
    let text = RichText::new(format!("{label} {value}"))
        .monospace()
        .strong()
        .color(color);
    ui.add(
        egui::Button::new(text)
            .fill(Color32::TRANSPARENT)
            .stroke(stroke)
            .corner_radius(3)
            .sense(egui::Sense::hover()),
    )
}

/// The trigger marker: a "T" in a small triangle on the top edge of the plot,
/// at t = 0, like on a scope screen. Off screen, an arrow at the edge points to it.
fn draw_trigger_marker(ui: &egui::Ui, transform: &PlotTransform) {
    let frame = *transform.frame();
    let x = transform.position_from_point_x(0.0);
    let painter = ui.painter_at(frame.expand(1.0));
    let top = frame.top();
    let font = egui::FontId::monospace(10.0);
    if x < frame.left() || x > frame.right() {
        let (x, text) = if x < frame.left() {
            (frame.left() + 12.0, "◀T")
        } else {
            (frame.right() - 12.0, "T▶")
        };
        painter.text(
            egui::pos2(x, top + 2.0),
            egui::Align2::CENTER_TOP,
            text,
            font,
            TRIGGER_COLOR,
        );
        return;
    }
    let (w, h) = (7.0, 12.0);
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(x - w, top),
            egui::pos2(x + w, top),
            egui::pos2(x, top + h),
        ],
        TRIGGER_COLOR,
        egui::Stroke::NONE,
    ));
    painter.text(
        egui::pos2(x, top),
        egui::Align2::CENTER_TOP,
        "T",
        font,
        Color32::BLACK,
    );
}

/// The OSD readout of a channel that is on, as a button that hides or shows
/// its trace. A hidden channel is grey with a dashed look ("hidden").
fn channel_button(
    ui: &mut egui::Ui,
    name: &str,
    text: &str,
    color: Color32,
    shown: bool,
) -> egui::Response {
    let (shown_color, suffix, tip) = if shown {
        (color, "", "Click to hide this channel")
    } else {
        (OFF_COLOR, " (hidden)", "Click to show this channel")
    };
    let label = RichText::new(format!("{name} {text}{suffix}"))
        .monospace()
        .strong()
        .color(shown_color);
    ui.add(
        egui::Button::new(label)
            .fill(Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.0, shown_color))
            .corner_radius(3),
    )
    .on_hover_text(tip)
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The contents of the Help window.
fn help_text(ui: &mut egui::Ui) {
    let section = |ui: &mut egui::Ui, title: &str, lines: &[&str]| {
        ui.add_space(6.0);
        ui.label(RichText::new(title).strong().color(Color32::WHITE));
        for line in lines {
            ui.label(format!("• {line}"));
        }
    };
    ui.label(
        RichText::new(format!("FNIRSI Viewer {VERSION} · commit {GIT_HASH}"))
            .small()
            .color(Color32::GRAY),
    );
    section(
        ui,
        "Open a file",
        &[
            "Drop a .wav file on the window, or click 📂 Open file….",
            "Supported: DPOX180H, and 1013D/1014D with the original FNIRSI firmware. The model is found automatically and shown at the top left.",
            "The file is read on this device; nothing is uploaded.",
            "If a file cannot be opened, a red banner says why, and the file shown before stays.",
        ],
    );
    section(
        ui,
        "Move and zoom",
        &[
            "Mouse wheel on the plot: zoom. Shift + wheel: time only. Alt + wheel: volts only.",
            "Mouse wheel on the time or voltage axis: move along that axis.",
            "Drag: move. Double-click: back to the start view.",
            "Touch: pinch to zoom, drag to move, double-tap to reset.",
            "⬌ Fit H: whole record. ⬍ Fit V: vertical axis fitted to what is on screen now. ⛶ Fit both: both.",
        ],
    );
    section(
        ui,
        "Vertical axis (Y: Volts / Div)",
        &[
            "Volts: one voltage axis for both channels.",
            "Div: like the scope screen, 8 divisions. Each channel uses its own V/div and starts from its ground marker.",
            "1▶ and 2▶ on the left edge: the 0 V level (ground) of each channel.",
            "The orange T on the top edge is the trigger (t = 0).",
        ],
    );
    section(
        ui,
        "Channels",
        &[
            "Click the CH1 or CH2 readout at the bottom to hide or show that channel.",
            "The readout shows V/div, probe (1X, 10X) and, if the file has it, the coupling (DC/AC).",
        ],
    );
    section(
        ui,
        "Cursors (⌖ Cursors menu)",
        &[
            "Time: two vertical lines T1, T2. Readout: T1, T2, ΔT, 1/ΔT, and each channel's voltage at T1 and T2.",
            "Volt: two horizontal lines V1, V2. Readout: V1, V2, ΔV (in Div mode, in the volts of each channel).",
            "Drag a dashed line to move it. Bring to view moves lost cursors back to the screen.",
        ],
    );
    section(
        ui,
        "Measurements (📏 Measure menu)",
        &[
            "Tick the measurements and channels to show. The choice is saved.",
            "Samples: Screen (the time span shown), Record (the whole file) or T1–T2 (between the time cursors).",
            "Values have 4 significant digits and never more resolution than the samples.",
            "?1.91 V in orange: the signal is clipped (it reaches the limit of the input range); the real value can be larger.",
            "<4.00 µs: the edge is faster than 2 samples; only an upper limit is known.",
            "—: the value cannot be measured (for example, fewer than two edges for a frequency).",
        ],
    );
    section(
        ui,
        "Keyboard",
        &["Esc: close this window, the welcome screen or an error message."],
    );
}

/// Name of a cursor mode, for the menu.
fn cursor_mode_name(mode: CursorMode) -> &'static str {
    match mode {
        CursorMode::Off => "Off",
        CursorMode::Time => "Time",
        CursorMode::Volt => "Volt",
        CursorMode::Both => "Both",
    }
}

/// Ground markers on the left edge of the plot, like on a scope screen: a
/// triangle with the channel number at the 0 V level of each channel.
/// Off screen, "1▲" or "1▼" at the top or bottom edge shows where it is.
fn draw_ground_markers(ui: &egui::Ui, transform: &PlotTransform, maps: &[Option<YMap>; 2]) {
    let frame = *transform.frame();
    let painter = ui.painter_at(frame.expand(1.0));
    let font = egui::FontId::monospace(10.0);
    let mut used: Vec<f32> = Vec::new();
    for (i, (map, color)) in maps.iter().zip([CH1_COLOR, CH2_COLOR]).enumerate() {
        let Some(map) = map else { continue };
        let label = format!("{}", i + 1);
        let y = transform.position_from_point_y(map.to_plot(0.0));
        // Two markers at about the same level: put the second beside the first.
        let dx = if used.iter().any(|u| (u - y).abs() < 12.0) {
            14.0
        } else {
            0.0
        };
        used.push(y);
        let left = frame.left() + dx;
        if y < frame.top() || y > frame.bottom() {
            let (text, pos, align) = if y < frame.top() {
                (
                    format!("{label}▲"),
                    egui::pos2(left + 2.0, frame.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                )
            } else {
                (
                    format!("{label}▼"),
                    egui::pos2(left + 2.0, frame.bottom() - 2.0),
                    egui::Align2::LEFT_BOTTOM,
                )
            };
            painter.text(pos, align, text, font.clone(), color);
            continue;
        }
        painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(left, y - 7.0),
                egui::pos2(left + 9.0, y - 7.0),
                egui::pos2(left + 14.0, y),
                egui::pos2(left + 9.0, y + 7.0),
                egui::pos2(left, y + 7.0),
            ],
            color,
            egui::Stroke::NONE,
        ));
        painter.text(
            egui::pos2(left + 5.0, y),
            egui::Align2::CENTER_CENTER,
            label,
            font.clone(),
            Color32::BLACK,
        );
    }
}

/// Text of one measurement of a channel, with fixed significant digits and
/// no more resolution than the samples have.
fn format_measurement(m: Measurement, value: Option<f64>, ch: &parser::Channel) -> String {
    let Some(v) = value else {
        return "—".to_owned();
    };
    let dt = ch.sample_interval().unwrap_or(0.0).abs();
    match m.unit() {
        // Mean and RMS average many samples, so they resolve less than one code.
        Unit::Volt => {
            let lsb = ch.lsb_volts();
            let res = if matches!(m, Measurement::Mean | Measurement::Rms) {
                lsb / 10.0
            } else {
                lsb
            };
            fmt_sig(v, "V", READOUT_DIGITS, res)
        }
        // A rise or fall faster than 2 samples cannot be measured.
        Unit::Second
            if matches!(m, Measurement::RiseTime | Measurement::FallTime) && v < 2.0 * dt =>
        {
            format!("<{}", fmt_sig(2.0 * dt, "s", 3, 0.0))
        }
        Unit::Second => fmt_sig(v, "s", READOUT_DIGITS, dt / 10.0),
        Unit::Hertz => fmt_sig(v, "Hz", READOUT_DIGITS, 0.0),
        Unit::Percent => format!("{v:.1} %"),
    }
}

/// Format a value with `digits` significant digits (trailing zeros kept, so
/// the width does not jump), an SI prefix and a unit. With `resolution > 0`,
/// the value is rounded to a multiple of it and no digit below it is shown.
/// Examples: `fmt_sig(1000.0, "Hz", 4, 0.0)` → `1.000 kHz`;
/// `fmt_sig(10e-6, "V", 4, 1e-3)` → `0 V`.
pub fn fmt_sig(value: f64, unit: &str, digits: i32, resolution: f64) -> String {
    if !value.is_finite() {
        return "—".to_owned();
    }
    let mut v = value;
    if resolution > 0.0 {
        v = (v / resolution).round() * resolution;
    }
    if v == 0.0 {
        return format!("0 {unit}");
    }
    // Round to `digits` significant digits first, so that the prefix fits the
    // rounded value (999.96 → 1.000 k, not 1000.0).
    let step = 10f64.powi(v.abs().log10().floor() as i32 - digits + 1);
    v = (v / step).round() * step;
    let (scale, prefix) = si_prefix(v.abs());
    let scaled = v / scale;
    let magnitude = scaled.abs().log10().floor() as i32;
    let mut decimals = (digits - 1 - magnitude).max(0);
    if resolution > 0.0 {
        let res_decimals = (-(resolution / scale).log10() - 1e-9).ceil() as i32;
        decimals = decimals.min(res_decimals.max(0));
    }
    format!("{scaled:.*} {prefix}{unit}", decimals as usize)
}

/// The SI prefix for a magnitude: (scale, prefix).
fn si_prefix(magnitude: f64) -> (f64, &'static str) {
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
    PREFIXES
        .iter()
        .find(|(s, _)| magnitude >= s * 0.9995)
        .copied()
        .unwrap_or((1e-15, "f"))
}

/// Format a value with an SI prefix and unit, for example `0.2, "V"` → `200 mV`.
pub fn si(value: f64, unit: &str) -> String {
    format!("{}{unit}", si_bare(value))
}

/// Format a value with an SI prefix and a trailing space, for example `4e8` → `400 M`.
pub fn si_bare(value: f64) -> String {
    if value == 0.0 || !value.is_finite() {
        return format!("{value} ");
    }
    let (scale, prefix) = si_prefix(value.abs());
    let text = format!("{:.3}", value / scale);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text} {prefix}")
}

#[cfg(test)]
mod tests {
    use super::{Axis, YMap, axis_zone, fmt_sig, si, time_span, voltage_span_in};
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
    fn divisions_mapping() {
        // 50 mV/div, ground 1.2 divisions below the centre.
        let m = YMap {
            scale: 1.0 / 0.05,
            offset: -1.2,
        };
        assert!(
            (m.to_plot(0.0) + 1.2).abs() < 1e-12,
            "0 V is at the ground marker"
        );
        assert!(
            (m.to_plot(0.1) - 0.8).abs() < 1e-12,
            "100 mV is 2 divisions above it"
        );
        assert!((m.to_volts(m.to_plot(0.037)) - 0.037).abs() < 1e-12);
    }

    #[test]
    fn fixed_significant_digits() {
        assert_eq!(fmt_sig(1000.0, "Hz", 4, 0.0), "1.000 kHz");
        assert_eq!(fmt_sig(99_980.0, "Hz", 4, 0.0), "99.98 kHz");
        assert_eq!(fmt_sig(0.99996, "V", 4, 0.0), "1.000 V");
        assert_eq!(fmt_sig(-30e-6, "s", 4, 0.0), "-30.00 µs");
        // Resolution: 1 mV per code. Less than half a code is 0 V.
        assert_eq!(fmt_sig(10e-6, "V", 4, 1e-3), "0 V");
        assert_eq!(fmt_sig(-0.99e-3, "V", 4, 1e-3), "-1 mV");
        assert_eq!(fmt_sig(0.5381, "V", 4, 1.25e-4), "538.1 mV");
        assert_eq!(fmt_sig(1.073, "V", 4, 1.25e-4), "1.073 V");
        assert_eq!(fmt_sig(f64::NAN, "V", 4, 0.0), "—");
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
