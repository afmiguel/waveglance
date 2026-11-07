//! Oscilloscope-style user interface.

use crate::cursors::{self, CursorMode, Cursors};
use crate::fft;
use crate::measure::{self, Measurement, Unit};
use crate::palette;
use crate::parser::{self, Acquisition, Waveform};
use crate::{examples, export};
use eframe::egui::{self, Color32, RichText};
use egui_plot::{HoverPosition, Line, LineStyle, Plot, PlotPoint, PlotTransform, VLine};
use std::borrow::Cow;
use std::ops::RangeInclusive;
use std::sync::mpsc::{Receiver, Sender, channel};

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
/// Storage key of the screen theme (white background).
const LIGHT_THEME_KEY: &str = "light_theme";
/// Storage key of the Math channel.
const MATH_KEY: &str = "math";
/// Storage key of the FFT settings.
const FFT_KEY: &str = "fft";
/// Part of the plot area used by the time plot when the FFT is shown.
const TIME_PLOT_SHARE: f32 = 0.55;
/// Min/max pairs drawn in the FFT plot.
const FFT_BUCKETS: usize = 2000;
/// Storage key of the zoom view (overview strip) switch.
const ZOOM_VIEW_KEY: &str = "zoom_view";
/// Height of the overview strip, in points.
const OVERVIEW_HEIGHT: f32 = 90.0;
/// Min/max pairs drawn in the overview (about one per pixel column).
const OVERVIEW_BUCKETS: usize = 1000;
/// Storage key of the vertical axis mode. ("y_axis" held the old default,
/// Divisions, saved automatically; a new key starts everyone in Volts.)
const Y_AXIS_KEY: &str = "y_axis_mode";
/// Vertical divisions on the scope screen (±4 around the centre).
const SCREEN_DIVS_Y: f64 = 8.0;
/// A channel whose amplitude is below this many divisions is noise: no
/// edges, period or widths are measured.
const MIN_AMPLITUDE_DIV: f64 = 0.1;

/// Where a file read by a dialog or a drop goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadTarget {
    /// The waveform to show and measure.
    Main,
    /// The reference waveform, drawn faint for comparison.
    Reference,
}

/// A file read by the open dialog or by drag-and-drop: (target, name, bytes or error).
type Loaded = (LoadTarget, String, Result<Vec<u8>, String>);

/// A reference waveform line: (name, plot points, colour).
type RefLine = (String, Vec<[f64; 2]>, Color32);

/// Math channel: a sum or difference of CH1 and CH2, sample by sample.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum MathOp {
    #[default]
    Off,
    Sum,
    Diff,
    DiffReverse,
}

impl MathOp {
    pub const ALL: [Self; 4] = [Self::Off, Self::Sum, Self::Diff, Self::DiffReverse];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Sum => "CH1 + CH2",
            Self::Diff => "CH1 − CH2",
            Self::DiffReverse => "CH2 − CH1",
        }
    }

    fn apply(self, a: f64, b: f64) -> f64 {
        match self {
            Self::Off => 0.0,
            Self::Sum => a + b,
            Self::Diff => a - b,
            Self::DiffReverse => b - a,
        }
    }
}

/// Event search (Search menu): what to look for, in which channel, the
/// times found and the event shown now.
#[derive(Debug, Clone, Default)]
struct Search {
    kind: Option<measure::EventKind>,
    /// 0 = CH1, 1 = CH2.
    source: usize,
    events: Vec<f64>,
    current: Option<usize>,
    /// What `events` was found from: (file, source, kind).
    found_for: Option<(u64, usize, measure::EventKind)>,
}

/// What a screenshot is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShotPurpose {
    /// Save it as a PNG file.
    Save,
    /// Print it (with the white screen).
    Print,
}

/// A screenshot waiting for its frame: menus close and the theme changes
/// first, so the picture shows only the plots and readouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShotJob {
    purpose: ShotPurpose,
    /// Frames to wait before the screenshot command.
    frames_left: u8,
    /// The command was sent; waiting for the image.
    sent: bool,
    /// The theme to go back to after the screenshot (true = white), if the
    /// screenshot used the other one.
    restore_light: Option<bool>,
}

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

/// A change of the main view asked for by the overview strip, applied in
/// the next frame of the main plot.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ViewCommand {
    /// Put this time at the centre; keep the width.
    CenterX(f64),
    /// Multiply the visible time span by `factor`, keeping time `at` fixed.
    ZoomX { factor: f64, at: f64 },
    /// Multiply the visible time span by `factor` around its centre (keys + and −).
    ZoomCenter(f64),
    /// Move the view by fractions of its width and height (arrow keys).
    Pan { x: f64, y: f64 },
}

/// FFT settings (saved between sessions).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FftSettings {
    /// Show the FFT panel under the time plot.
    pub show: bool,
    /// Source channel: 0 = CH1, 1 = CH2.
    pub source: usize,
    pub window: fft::Window,
    /// Level in dBV (true) or in volts RMS (false).
    pub db: bool,
}

impl Default for FftSettings {
    fn default() -> Self {
        Self {
            show: false,
            source: 0,
            window: fft::Window::Hanning,
            db: true,
        }
    }
}

/// What a computed spectrum depends on: file, channel, window and the time
/// span (as bits, so that the key can be compared exactly).
type FftKey = (u64, usize, fft::Window, u64, u64);

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
    /// Zoom view: an overview of the whole record above the main plot.
    zoom_view: bool,
    /// View change from the overview strip, for the next frame.
    pending_view: Option<ViewCommand>,
    /// White screen (for printing) instead of black.
    light_theme: bool,
    /// Div mode: how far the user moved each channel (divisions), by dragging
    /// its ground marker. Reset when a file is opened.
    ground_shift: [f64; 2],
    /// The ground marker that the pointer is dragging (0 = CH1, 1 = CH2).
    marker_drag: Option<usize>,
    /// Reference waveform (file name, waveform), drawn faint behind the traces.
    reference: Option<(String, Waveform)>,
    /// Show the reference waveform.
    show_reference: bool,
    /// A screenshot in progress (Save screenshot, Print).
    shot: Option<ShotJob>,
    /// The Print window (choice of white or black screen) is open.
    print_dialog: bool,
    /// Print on the white screen (the choice in the Print window).
    print_white: bool,
    /// Math channel operation (saved).
    math: MathOp,
    /// Show the Math channel trace.
    math_visible: bool,
    /// Event search.
    search: Search,
    /// Screenshots arrive here (egui calls back after the frame is drawn).
    shot_tx: Sender<std::sync::Arc<egui::ColorImage>>,
    shot_rx: Receiver<std::sync::Arc<egui::ColorImage>>,
    /// Tab presses taken from egui while cursors are on (they select the
    /// next cursor instead of moving the keyboard focus between buttons).
    tab_presses: usize,
    /// FFT panel settings.
    fft: FftSettings,
    /// The last spectrum and what it was computed from.
    fft_cache: Option<(FftKey, Option<fft::Spectrum>)>,
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
        // Both themes: the plot background is the scope screen colour.
        let mut dark = egui::Visuals::dark();
        dark.extreme_bg_color = egui::Color32::from_rgb(10, 10, 10);
        cc.egui_ctx.set_visuals_of(egui::Theme::Dark, dark);
        let mut light = egui::Visuals::light();
        light.extreme_bg_color = egui::Color32::WHITE;
        cc.egui_ctx.set_visuals_of(egui::Theme::Light, light);
        let light_theme: bool = cc
            .storage
            .and_then(|s| eframe::get_value(s, LIGHT_THEME_KEY))
            .unwrap_or(false);
        cc.egui_ctx.set_theme(if light_theme {
            egui::Theme::Light
        } else {
            egui::Theme::Dark
        });

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
        let (shot_tx, shot_rx) = channel();
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
            zoom_view: cc
                .storage
                .and_then(|s| eframe::get_value(s, ZOOM_VIEW_KEY))
                .unwrap_or(false),
            pending_view: None,
            fft: cc
                .storage
                .and_then(|s| eframe::get_value(s, FFT_KEY))
                .unwrap_or_default(),
            fft_cache: None,
            light_theme,
            ground_shift: [0.0; 2],
            marker_drag: None,
            tab_presses: 0,
            reference: None,
            show_reference: true,
            shot: None,
            print_dialog: false,
            print_white: true,
            math: cc
                .storage
                .and_then(|s| eframe::get_value(s, MATH_KEY))
                .unwrap_or_default(),
            math_visible: true,
            search: Search::default(),
            shot_tx,
            shot_rx,
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
                self.ground_shift = [0.0; 2];
                self.marker_drag = None;
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
                    YAxis::Divisions => {
                        let mut m = YMap::divisions(c);
                        m.offset += self.ground_shift[i];
                        m
                    }
                    YAxis::Volts => YMap::IDENTITY,
                })
        };
        [map(&w.ch1, 0), map(&w.ch2, 1)]
    }

    /// The Math channel, computed sample by sample from CH1 and CH2 (both
    /// must be on, with the same samples). Its V/div is the larger of the two.
    pub fn math_channel(&self) -> Option<parser::Channel> {
        if self.math == MathOp::Off {
            return None;
        }
        let w = self.waveform.as_ref()?;
        let (a, b) = (w.ch1.as_ref()?, w.ch2.as_ref()?);
        if a.points.len() != b.points.len() {
            return None;
        }
        let points = a
            .points
            .iter()
            .zip(&b.points)
            .map(|(p, q)| [p[0], self.math.apply(p[1], q[1])])
            .collect();
        let volts_per_div = a.volts_per_div.max(b.volts_per_div);
        Some(parser::Channel {
            volts_per_div,
            probe_factor: 1.0,
            ground_adc: 0.0,
            // One code: the finer of the two channels.
            counts_per_div: volts_per_div / a.lsb_volts().min(b.lsb_volts()),
            center_code: 0.0,
            code_max: u16::MAX,
            coupling: None,
            raw: Vec::new(),
            points,
            clipped: a.clipped || b.clipped,
        })
    }

    /// Plot mapping of the Math channel (`None` if off or hidden).
    fn math_map(&self, math: Option<&parser::Channel>) -> Option<YMap> {
        let m = math.filter(|_| self.math_visible)?;
        Some(match self.y_axis {
            YAxis::Divisions => YMap {
                scale: 1.0 / m.volts_per_div,
                offset: 0.0,
            },
            YAxis::Volts => YMap::IDENTITY,
        })
    }

    /// Contents of the Math menu.
    fn math_menu(&mut self, ui: &mut egui::Ui) {
        let both = self
            .waveform
            .as_ref()
            .is_some_and(|w| w.ch1.is_some() && w.ch2.is_some());
        for op in MathOp::ALL {
            let enabled = op == MathOp::Off || both;
            if ui
                .add_enabled(enabled, egui::RadioButton::new(self.math == op, op.label()))
                .on_disabled_hover_text("Needs CH1 and CH2 on in the file")
                .clicked()
            {
                self.math = op;
                self.math_visible = true;
                ui.close();
            }
        }
        ui.separator();
        ui.label(
            RichText::new("Sample by sample, in volts. Measured like a channel (MATH row).")
                .small()
                .color(palette::hint()),
        );
    }

    /// Contents of the Search menu.
    fn search_menu(&mut self, ui: &mut egui::Ui) {
        let present = self
            .waveform
            .as_ref()
            .map_or([false, false], |w| [w.ch1.is_some(), w.ch2.is_some()]);
        let mut kind = self.search.kind;
        ui.radio_value(&mut kind, None, "Off");
        for k in measure::EventKind::ALL {
            ui.radio_value(&mut kind, Some(k), k.label()).on_hover_text(match k {
                measure::EventKind::Glitch => {
                    "Pulses much shorter than the others (shorter than 10% of the typical time between edges)"
                }
                _ => "At the 50% level between Vbase and Vtop, with hysteresis (10%–90%)",
            });
        }
        if kind != self.search.kind {
            self.search.kind = kind;
            self.search.current = None;
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("In:");
            for (i, (name, color)) in [("CH1", palette::ch1()), ("CH2", palette::ch2())]
                .into_iter()
                .enumerate()
            {
                if ui
                    .add_enabled(
                        present[i],
                        egui::RadioButton::new(
                            self.search.source == i,
                            RichText::new(name).color(color),
                        ),
                    )
                    .clicked()
                {
                    self.search.source = i;
                    self.search.current = None;
                }
            }
        });
        if self.search.kind.is_some() {
            ui.separator();
            ui.label(format!(
                "{} found in the whole record",
                self.search.events.len()
            ));
            ui.horizontal(|ui| {
                if ui.button("◀ Previous").on_hover_text("Key [").clicked() {
                    self.search_step(-1);
                }
                if ui.button("Next ▶").on_hover_text("Key ]").clicked() {
                    self.search_step(1);
                }
            });
        }
    }

    /// Find the events again when the file, channel or kind changed.
    fn update_search(&mut self) {
        let Some(kind) = self.search.kind else {
            self.search.events.clear();
            self.search.found_for = None;
            return;
        };
        let key = (self.load_count, self.search.source, kind);
        if self.search.found_for == Some(key) {
            return;
        }
        self.search.found_for = Some(key);
        self.search.current = None;
        self.search.events = self
            .waveform
            .as_ref()
            .and_then(|w| [&w.ch1, &w.ch2][self.search.source].as_ref())
            .map(|ch| measure::find_events(&ch.points, kind, MIN_AMPLITUDE_DIV * ch.volts_per_div))
            .unwrap_or_default();
    }

    /// Go to the next (`step` = 1) or previous (−1) event and centre the view on it.
    fn search_step(&mut self, step: i64) {
        let n = self.search.events.len();
        if n == 0 {
            return;
        }
        let next = match self.search.current {
            Some(i) => (i as i64 + step).rem_euclid(n as i64) as usize,
            None => {
                // The first event after (or before) the centre of the view.
                let centre = self.last_transform.as_ref().map_or(0.0, |t| {
                    let r = t.bounds().range_x();
                    (r.start() + r.end()) / 2.0
                });
                let after = self.search.events.partition_point(|&t| t <= centre);
                if step > 0 {
                    after % n
                } else {
                    (after + n - 1) % n
                }
            }
        };
        self.search.current = Some(next);
        self.pending_view = Some(ViewCommand::CenterX(self.search.events[next]));
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

    /// Contents of the Y menu: Volts (default) / Div, as radio buttons.
    fn y_axis_buttons(&mut self, ui: &mut egui::Ui) {
        let mut y_axis = self.y_axis;
        ui.radio_value(&mut y_axis, YAxis::Volts, "Volts")
            .on_hover_text("Vertical axis in volts: both channels share the axis");
        ui.radio_value(&mut y_axis, YAxis::Divisions, "Div").on_hover_text(
            "Vertical axis in divisions, like the scope screen: each channel has its own V/div and ground marker",
        );
        if y_axis != self.y_axis {
            self.set_y_axis(y_axis);
            ui.close();
        }
        if self.ground_shift != [0.0; 2] {
            ui.separator();
            if ui
                .button("Reset channel positions")
                .on_hover_text("Put the ground markers back where the scope had them")
                .clicked()
            {
                self.ground_shift = [0.0; 2];
                ui.close();
            }
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
        let modal =
            egui::Modal::new(egui::Id::unique("splash")).show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.heading("FNIRSI Viewer");
                ui.label(
                    RichText::new(format!("Version {VERSION} · commit {GIT_HASH}"))
                        .small()
                        .color(palette::hint()),
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
                        ("Drag 1▶ / 2▶", "Move that channel up or down (switches Y to Div)"),
                        ("Click CH1 / CH2 readout", "Hide or show the channel"),
                        ("Touch", "Pinch: zoom · drag: pan · double-tap: reset"),
                        (
                            "Keys",
                            "F fit · Z zoom view · S FFT · C cursors · Tab + arrows move a cursor",
                        ),
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
                        .color(palette::hint()),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("⏏ Open file…").clicked() {
                        open_file = true;
                    }
                    if ui.button("? Help").clicked() {
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
            ui.add(
                egui::Label::new(RichText::new(format!("⚠ {err}")).color(palette::error())).wrap(),
            );
        });
        if close || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.error = None;
        }
    }

    fn receive_loaded_files(&mut self) {
        while let Ok((target, name, result)) = self.inbox_rx.try_recv() {
            match (target, result) {
                (LoadTarget::Main, Ok(bytes)) => self.load_bytes(name, &bytes),
                (LoadTarget::Reference, Ok(bytes)) => self.load_reference(name, &bytes),
                (_, Err(e)) => self.report_open_error(&name, &e),
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
            let _ = self.inbox_tx.send((LoadTarget::Main, name, result));
        }
        #[cfg(target_arch = "wasm32")]
        {
            let tx = self.inbox_tx.clone();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = file.bytes_async().await;
                let _ = tx.send((LoadTarget::Main, name, result));
                ctx.request_repaint();
            });
        }
    }

    /// Show the "Open file" dialog (`rfd`) for the main waveform.
    fn open_file_dialog(&mut self, ctx: &egui::Context) {
        self.open_dialog(ctx, LoadTarget::Main);
    }

    /// Show the "Open file" dialog (`rfd`); the file goes to `target`.
    fn open_dialog(&mut self, ctx: &egui::Context, target: LoadTarget) {
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
                let _ = self.inbox_tx.send((target, name, result));
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
                    let _ = tx.send((target, file.file_name(), Ok(bytes)));
                    ctx.request_repaint();
                }
            });
        }
    }

    /// The reference waveform as plot lines (name, points, colour): mapped
    /// like the main traces (Volts: the same axis; Div: its own V/div and
    /// ground), and reduced for drawing.
    fn reference_lines(&self) -> [Option<RefLine>; 2] {
        let Some((_, w)) = self.reference.as_ref().filter(|_| self.show_reference) else {
            return [None, None];
        };
        std::array::from_fn(|i| {
            let ch = [&w.ch1, &w.ch2][i].as_ref()?;
            let map = match self.y_axis {
                YAxis::Divisions => YMap::divisions(ch),
                YAxis::Volts => YMap::IDENTITY,
            };
            // Only inside the time span of the main file, so that the
            // reference never changes the view (auto bounds, Fit, reset).
            let span = time_span(&[
                self.channel_1_data.as_slice(),
                self.channel_2_data.as_slice(),
            ])?;
            let points: Vec<[f64; 2]> = ch
                .points
                .iter()
                .filter(|p| span.contains(&p[0]))
                .map(|p| [p[0], map.to_plot(p[1])])
                .collect();
            Some((
                format!("REF CH{}", i + 1),
                decimate(&points, 4 * OVERVIEW_BUCKETS),
                palette::reference(i),
            ))
        })
    }

    /// Parse a reference file. A bad file gives the error banner and keeps
    /// the reference shown before.
    fn load_reference(&mut self, name: String, bytes: &[u8]) {
        match parser::parse(bytes) {
            Ok(w) => {
                self.reference = Some((name, w));
                self.show_reference = true;
            }
            Err(e) => {
                self.error = Some(format!("Could not open reference {name}: {e}."));
            }
        }
    }

    /// Save the samples of the main waveform as CSV.
    fn export_csv(&mut self) {
        let (Some(w), Some(name)) = (&self.waveform, &self.file_name) else {
            return;
        };
        let text = export::csv(w, name, VERSION);
        let file = format!("{}.csv", export::stem(name));
        if let Err(e) = export::save(&file, text.as_bytes(), "text/csv", ("CSV", "csv")) {
            self.error = Some(format!("Could not save {file}: {e}."));
        }
    }

    /// Start a screenshot (for a PNG file or for printing). Printing uses the
    /// white screen to save ink.
    /// `light`: the theme for the picture (`None`: the theme in use). The
    /// theme in use comes back after the picture.
    fn start_shot(&mut self, ctx: &egui::Context, purpose: ShotPurpose, light: Option<bool>) {
        let restore_light = match light {
            Some(l) if l != self.light_theme => {
                let before = self.light_theme;
                self.set_light_theme(ctx, l);
                Some(before)
            }
            _ => None,
        };
        self.shot = Some(ShotJob {
            purpose,
            frames_left: 3,
            sent: false,
            restore_light,
        });
        ctx.request_repaint();
    }

    /// Advance the screenshot in progress: wait a few frames, send the
    /// command, then save or print the image when it arrives.
    fn update_shot(&mut self, ctx: &egui::Context) {
        let Some(mut job) = self.shot else { return };
        if !job.sent {
            if job.frames_left > 0 {
                job.frames_left -= 1;
            } else {
                let tx = self.shot_tx.clone();
                let repaint = ctx.clone();
                ctx.request_screenshot(move |image| {
                    let _ = tx.send(image);
                    repaint.request_repaint();
                });
                job.sent = true;
            }
            self.shot = Some(job);
            ctx.request_repaint();
            return;
        }
        let Ok(image) = self.shot_rx.try_recv() else {
            return;
        };
        self.shot = None;
        if let Some(light) = job.restore_light {
            self.set_light_theme(ctx, light);
        }
        let base = self
            .file_name
            .as_deref()
            .map_or("fnirsi-viewer", export::stem);
        let result = export::png(&image).and_then(|png| match job.purpose {
            ShotPurpose::Save => export::save(
                &format!("{base}.png"),
                &png,
                "image/png",
                ("PNG image", "png"),
            )
            .map(|_| ()),
            ShotPurpose::Print => export::print_png(&png),
        });
        if let Err(e) = result {
            self.error = Some(format!("Could not save or print the picture: {e}."));
        }
    }

    /// The Print window: white (recommended) or black screen, then print.
    fn print_window(&mut self, ctx: &egui::Context) {
        if !self.print_dialog {
            return;
        }
        let mut go = false;
        let modal = egui::Modal::new(egui::Id::unique("print_dialog")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading("Print");
            ui.add_space(4.0);
            ui.radio_value(
                &mut self.print_white,
                true,
                "White background (recommended)",
            );
            ui.label(
                RichText::new(
                    "Uses much less ink or toner, and the traces use darker colours that \
                     stay easy to read on paper.",
                )
                .small()
                .color(palette::hint()),
            );
            ui.add_space(4.0);
            ui.radio_value(&mut self.print_white, false, "Black background");
            ui.label(
                RichText::new(
                    "Like the scope screen. Prints a lot of black: much more ink or toner.",
                )
                .small()
                .color(palette::hint()),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("The screen changes only for the picture; it comes back by itself.")
                    .small()
                    .color(palette::hint()),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Print…").clicked() {
                    go = true;
                }
                if ui.button("Cancel").clicked() {
                    self.print_dialog = false;
                }
            });
        });
        if modal.should_close() {
            self.print_dialog = false;
        }
        if go {
            self.print_dialog = false;
            let light = self.print_white;
            self.start_shot(ctx, ShotPurpose::Print, Some(light));
        }
    }

    /// Switch between the black and the white screen.
    fn set_light_theme(&mut self, ctx: &egui::Context, light: bool) {
        self.light_theme = light;
        palette::set_light(light);
        ctx.set_theme(if light {
            egui::Theme::Light
        } else {
            egui::Theme::Dark
        });
    }

    /// Contents of the File menu.
    fn file_menu(&mut self, ui: &mut egui::Ui) {
        let loaded = self.waveform.is_some();
        if ui
            .button("Open file…")
            .on_hover_text("Open a FNIRSI .wav file (Ctrl/⌘+O), or drop it on the window")
            .clicked()
        {
            ui.close();
            self.open_file_dialog(ui.ctx());
        }
        ui.menu_button("Open example", |ui| {
            for e in examples::EXAMPLES {
                if ui.button(e.label).on_hover_text(e.about).clicked() {
                    self.load_bytes(e.file.to_owned(), e.bytes);
                    ui.close();
                }
            }
            ui.separator();
            ui.label(
                RichText::new(examples::CREDIT)
                    .small()
                    .color(palette::hint()),
            );
        });
        ui.separator();
        if ui
            .add_enabled(loaded, egui::Button::new("Open reference…"))
            .on_hover_text("Open a second file, drawn faint behind the traces, to compare with")
            .clicked()
        {
            ui.close();
            self.open_dialog(ui.ctx(), LoadTarget::Reference);
        }
        if let Some((name, _)) = &self.reference {
            let label = format!("Show reference ({name})");
            ui.checkbox(&mut self.show_reference, label);
            if ui.button("Close reference").clicked() {
                self.reference = None;
                ui.close();
            }
        }
        ui.separator();
        if ui
            .add_enabled(loaded, egui::Button::new("Export CSV…"))
            .on_hover_text("Save the samples as a table: time and the voltage of each channel")
            .clicked()
        {
            ui.close();
            self.export_csv();
        }
        if ui
            .add_enabled(loaded, egui::Button::new("Save screenshot (PNG)…"))
            .on_hover_text("Save a picture of the plots and readouts, as shown")
            .clicked()
        {
            ui.close();
            self.start_shot(ui.ctx(), ShotPurpose::Save, None);
        }
        if ui
            .add_enabled(loaded, egui::Button::new("Print…"))
            .on_hover_text("Print the plots and readouts on a white screen (Ctrl/⌘+P)")
            .clicked()
        {
            ui.close();
            self.print_dialog = true;
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            // Button text never wraps: a button that does not fit moves to the
            // next row as a whole.
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            // The model comes from the loaded file.
            let title = match &self.waveform {
                Some(w) => format!("FNIRSI {}", w.model.name()),
                None => "FNIRSI Viewer".to_owned(),
            };
            ui.label(RichText::new(title).strong().color(palette::fg_dim()))
                .on_hover_text("Supported: DPOX180H, 1013D and 1014D (original firmware)");
            // Version, discreet: identifies the build (also between releases).
            ui.label(
                RichText::new(format!("v{VERSION}"))
                    .small()
                    .color(palette::hint()),
            )
            .on_hover_text(format!("fnirsi-viewer {VERSION}, commit {GIT_HASH}"));
            ui.add_space(8.0);
            ui.menu_button("⏏ File ▾", |ui| {
                ui.style_mut().wrap_mode = None;
                self.file_menu(ui);
            })
            .response
            .on_hover_text("Open, examples, reference, export, print");
            let loaded = self.waveform.is_some();
            // All the options are menu buttons with ▾, so the bar is the same
            // on a wide and on a narrow (phone) screen. Each one is a single
            // widget (no enabled/disabled group around it), so that a button
            // that does not fit moves to the next row.
            let stay_open = || {
                Some(
                    egui::containers::menu::MenuConfig::new()
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                )
            };
            ui.add_space(8.0);
            toolbar_menu(
                ui,
                loaded,
                "⛶ Fit ▾".to_owned(),
                "Fit the view to the waveform",
                None,
                |ui| self.fit_buttons(ui),
            );
            ui.add_space(8.0);
            let y_label = match self.y_axis {
                YAxis::Volts => "Y: Volts ▾",
                YAxis::Divisions => "Y: Div ▾",
            };
            toolbar_menu(
                ui,
                true,
                y_label.to_owned(),
                "Vertical axis: volts, or divisions like the scope screen",
                None,
                |ui| self.y_axis_buttons(ui),
            );
            ui.add_space(8.0);
            if ui
                .add_enabled(loaded, egui::Button::selectable(self.zoom_view, "⊕ Zoom"))
                .on_hover_text(
                    "Zoom view: an overview of the whole record above the plot. \
                     Click or drag in it to move, wheel to zoom.",
                )
                .clicked()
            {
                self.zoom_view = !self.zoom_view;
            }
            ui.add_space(8.0);
            let fft_label = if self.fft.show {
                format!("∿ FFT: CH{} ▾", self.fft_source().map_or(1, |i| i + 1))
            } else {
                "∿ FFT ▾".to_owned()
            };
            toolbar_menu(
                ui,
                loaded,
                fft_label,
                "Spectrum of a channel (FFT) under the plot",
                stay_open(),
                |ui| self.fft_menu(ui),
            );
            ui.add_space(8.0);
            // Cursor menu: the button shows the mode in use.
            let cursor_label = match self.cursors.mode {
                CursorMode::Off => "⌖ Cursors ▾".to_owned(),
                m => format!("⌖ Cursors: {} ▾", cursor_mode_name(m)),
            };
            toolbar_menu(
                ui,
                loaded,
                cursor_label,
                "Measurement cursors",
                None,
                |ui| self.cursor_menu(ui),
            );
            ui.add_space(8.0);
            // Always enabled: the choice can be made before a file is loaded.
            // Stay open while items are ticked; close on a click outside the menu.
            toolbar_menu(
                ui,
                true,
                "▤ Measure ▾".to_owned(),
                "Choose the automatic measurements to show",
                stay_open(),
                |ui| self.measure_menu(ui),
            );
            ui.add_space(8.0);
            let math_label = if self.math == MathOp::Off {
                "Σ Math ▾".to_owned()
            } else {
                format!("Σ Math: {} ▾", self.math.label())
            };
            toolbar_menu(
                ui,
                loaded,
                math_label,
                "A channel computed from CH1 and CH2",
                None,
                |ui| self.math_menu(ui),
            );
            ui.add_space(8.0);
            let search_label = match self.search.kind {
                Some(k) => format!("⌕ Search: {} ▾", k.label()),
                None => "⌕ Search ▾".to_owned(),
            };
            toolbar_menu(
                ui,
                loaded,
                search_label,
                "Find edges or glitches in the whole record",
                stay_open(),
                |ui| self.search_menu(ui),
            );
            ui.add_space(8.0);
            let (theme_label, theme_tip) = if self.light_theme {
                ("◐ Black", "Black screen, like the scope")
            } else {
                ("◐ White", "White screen with darker traces, for printing")
            };
            if ui.button(theme_label).on_hover_text(theme_tip).clicked() {
                self.set_light_theme(ui.ctx(), !self.light_theme);
            }
            ui.add_space(8.0);
            if ui
                .button("? Help")
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
                .color(palette::hint()),
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
                ("CH1", self.ch1_v_div, &ch1_probe, palette::ch1()),
                ("CH2", self.ch2_v_div, &ch2_probe, palette::ch2()),
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
            readout(ui, "H", self.timebase.map(|t| format!("{}/div", si(t, "s"))), palette::fg());

            let Some(w) = &self.waveform else { return };
            let mode = match w.acquisition {
                Acquisition::RealTime { sample_rate_hz } => format!("{}Sa/s", si_bare(sample_rate_hz)),
                Acquisition::Equivalent { frames } => format!("ETS · frame 1 of {frames}"),
            };
            readout(ui, "MODE", Some(mode), palette::fg_dim());
            if let Some(t) = &w.trigger {
                let edge = if t.rising { "↑" } else { "↓" };
                readout(
                    ui,
                    "TRIG",
                    Some(format!("{} {edge} CH{}", t.mode, t.source)),
                    palette::trigger(),
                );
            }
            readout(ui, "PTS", Some(w.samples_per_channel.to_string()), palette::fg_dim());
            // Math channel: a click hides or shows it.
            if let Some(m) = self.math_channel() {
                let text = format!("{} {}/div", self.math.label(), si(m.volts_per_div, "V"));
                if channel_button(ui, "MATH", &text, palette::math(), self.math_visible).clicked() {
                    self.math_visible = !self.math_visible;
                }
            }
            if let Some(kind) = self.search.kind {
                let n = self.search.events.len();
                let at = self.search.current.map_or("–".to_owned(), |i| (i + 1).to_string());
                readout(
                    ui,
                    "SEARCH",
                    Some(format!("{} CH{} {at}/{n}", kind.label(), self.search.source + 1)),
                    palette::fg_dim(),
                )
                .on_hover_text("Search menu: [ and ] go to the previous and next event");
            }
            // Reference: a click hides or shows it, like a channel.
            if let Some((name, _)) = &self.reference {
                let text = format!("REF {name}");
                let color = if self.show_reference {
                    palette::reference(0)
                } else {
                    palette::off()
                };
                if ui
                    .add(
                        egui::Button::new(RichText::new(text).monospace().strong().color(color))
                            .fill(Color32::TRANSPARENT)
                            .stroke(egui::Stroke::new(1.0, color))
                            .corner_radius(3),
                    )
                    .on_hover_text("Reference waveform (File ▾ → Open reference…). Click to hide or show it.")
                    .clicked()
                {
                    self.show_reference = !self.show_reference;
                }
            }

            let clipped: Vec<&str> = [("CH1", &w.ch1), ("CH2", &w.ch2)]
                .iter()
                .filter(|(_, c)| c.as_ref().is_some_and(|c| c.clipped))
                .map(|(n, _)| *n)
                .collect();
            if !clipped.is_empty() {
                ui.label(RichText::new(format!("⚠ {} clipped", clipped.join(", "))).color(palette::error()))
                    .on_hover_text("The signal reaches the limit of the input range: amplitude measurements are marked with \"?\"");
            }
            for warning in &w.warnings {
                ui.label(RichText::new(format!("⚠ {warning}")).color(palette::warn_text()));
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
            for (i, (name, color)) in [("CH1", palette::ch1()), ("CH2", palette::ch2())]
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
        ui.checkbox(&mut m.stats, "Statistics per cycle")
            .on_hover_text(
                "A window with mean, min, max, σ and number of cycles of the time measurements",
            );
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

    /// The channels to measure: (name, channel, colour, clipped in the
    /// window). CH1/CH2 when on, shown and ticked; MATH when on and shown.
    fn measured_channels(
        &self,
        window: &RangeInclusive<f64>,
    ) -> Vec<(&'static str, Cow<'_, parser::Channel>, Color32, bool)> {
        let mut rows = Vec::new();
        let Some(w) = &self.waveform else { return rows };
        for (i, (name, ch, color)) in [
            ("CH1", &w.ch1, palette::ch1()),
            ("CH2", &w.ch2, palette::ch2()),
        ]
        .into_iter()
        .enumerate()
        {
            let Some(ch) = ch else { continue };
            if self.measure.channels[i] && self.visible[i] {
                rows.push((name, Cow::Borrowed(ch), color, ch.clipped_in(window)));
            }
        }
        if let Some(m) = self.math_channel().filter(|_| self.math_visible) {
            // Clipped if a source channel is clipped in the window.
            let clipped = [&w.ch1, &w.ch2]
                .iter()
                .any(|c| c.as_ref().is_some_and(|c| c.clipped_in(window)));
            rows.push(("MATH", Cow::Owned(m), palette::math(), clipped));
        }
        rows
    }

    /// Automatic measurement readouts, one row per channel.
    fn measure_readout(&self, ui: &mut egui::Ui) {
        let Some((window, window_name)) = self.measure_window() else {
            return;
        };
        for (name, ch, color, clipped) in self.measured_channels(&window) {
            let ch = ch.as_ref();
            let r = measure::measure(&ch.points, &window, MIN_AMPLITUDE_DIV * ch.volts_per_div);
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
                        readout(ui, m.label(), Some(format!("?{value}")), palette::warn()).on_hover_text(
                            format!("{}\n⚠ {name} is clipped in this window: the real value can be larger", m.description()),
                        );
                    } else {
                        readout(ui, m.label(), Some(value), color).on_hover_text(m.description());
                    }
                }
            });
        }
    }

    /// The statistics window (Measure menu → Statistics per cycle): one table
    /// per measured channel. A floating window, so that it never takes the
    /// space of the plot.
    fn stats_window(&mut self, ctx: &egui::Context) {
        if !self.measure.stats || self.waveform.is_none() {
            return;
        }
        let Some((window, window_name)) = self.measure_window() else {
            return;
        };
        let mut open = true;
        egui::Window::new("Statistics per cycle")
            .open(&mut open)
            .default_pos(ctx.content_rect().right_top() + egui::vec2(-470.0, 70.0))
            .default_width(440.0)
            .resizable(true)
            .collapsible(true)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!(
                        "Each cycle in the {window_name} window: mean, min, max, standard deviation σ and number n."
                    ))
                    .small()
                    .color(palette::hint()),
                );
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut any = false;
                    for (name, ch, color, _) in self.measured_channels(&window) {
                        let ch = ch.as_ref();
                        let r = measure::measure(&ch.points, &window, MIN_AMPLITUDE_DIV * ch.volts_per_div);
                        any |= stats_table(ui, name, &r, ch, color, &self.measure);
                        ui.add_space(6.0);
                    }
                    if !any {
                        ui.label("Tick a time measurement (Freq, Period, Rise, Fall, ±Width, Duty) in the Measure menu.");
                    }
                });
            });
        if !open {
            self.measure.stats = false;
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
        !self.measure.selected.is_empty()
            && ((0..2).any(|i| on[i] && self.measure.channels[i])
                || (self.math != MathOp::Off && self.math_visible))
    }

    /// Cursor readouts: positions, differences and channel values at T1/T2.
    fn cursor_readout(&self, ui: &mut egui::Ui) {
        let c = &self.cursors;
        if c.mode.shows_time() {
            ui.horizontal_wrapped(|ui| {
                let color = palette::time_cursor();
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
                for (i, (name, ch, color)) in [
                    ("CH1", &w.ch1, palette::ch1()),
                    ("CH2", &w.ch2, palette::ch2()),
                ]
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
                let color = palette::volt_cursor();
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
                for (i, (name, ch_color)) in [("CH1", palette::ch1()), ("CH2", palette::ch2())]
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

    /// Div mode: start, follow and end the drag of a ground marker.
    fn handle_marker_drag(&mut self, ctx: &egui::Context, on_plot: bool) {
        let (pos, pressed, down) = ctx.input(|i| {
            (
                i.pointer.latest_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
            )
        });
        if !down {
            self.marker_drag = None;
            return;
        }
        let (Some(pos), Some(t)) = (pos, self.last_transform) else {
            return;
        };
        if pressed && on_plot && self.marker_drag.is_none() && self.cursors.dragging.is_none() {
            self.marker_drag = marker_at(&t, &self.y_maps(), pos);
            // Volts mode: both channels share one axis, so a channel cannot
            // move alone. A drag on a marker switches to Div, where it can.
            if self.marker_drag.is_some() && self.y_axis == YAxis::Volts {
                self.set_y_axis(YAxis::Divisions);
                return;
            }
        }
        if self.y_axis != YAxis::Divisions {
            self.marker_drag = None;
            return;
        }
        let Some(i) = self.marker_drag else { return };
        let Some(w) = &self.waveform else { return };
        let Some(ch) = [&w.ch1, &w.ch2][i] else {
            return;
        };
        let base = YMap::divisions(ch).offset;
        let y = t.value_from_position(pos).y;
        self.ground_shift[i] = y - base;
    }

    /// Keyboard shortcuts. Keys are taken unless a text field is typed in.
    fn handle_keys(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        if ctx.text_edit_focused() {
            return;
        }
        let key = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_key(m, k));
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::O)))
        {
            self.open_file_dialog(ctx);
        }
        if key(Modifiers::NONE, Key::F1) {
            self.help_open = !self.help_open;
        }
        if key(Modifiers::NONE, Key::B) {
            self.set_light_theme(ctx, !self.light_theme);
        }
        if self.waveform.is_some()
            && self.shot.is_none()
            && ctx.input_mut(|i| {
                i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::P))
            })
        {
            self.print_dialog = true;
        }
        if self.waveform.is_none() {
            return;
        }
        for (k, fit) in [
            (Key::F, Fit::Both),
            (Key::H, Fit::Horizontal),
            (Key::V, Fit::Vertical),
        ] {
            if key(Modifiers::NONE, k) {
                self.pending_fit = Some(fit);
            }
        }
        if key(Modifiers::NONE, Key::Z) {
            self.zoom_view = !self.zoom_view;
        }
        if key(Modifiers::NONE, Key::S) {
            self.fft.show = !self.fft.show;
        }
        if key(Modifiers::NONE, Key::Y) {
            self.set_y_axis(match self.y_axis {
                YAxis::Volts => YAxis::Divisions,
                YAxis::Divisions => YAxis::Volts,
            });
        }
        if key(Modifiers::NONE, Key::C) {
            self.cursors.set_mode(match self.cursors.mode {
                CursorMode::Off => CursorMode::Time,
                CursorMode::Time => CursorMode::Volt,
                CursorMode::Volt => CursorMode::Both,
                CursorMode::Both => CursorMode::Off,
            });
        }
        for (i, k) in [Key::Num1, Key::Num2].into_iter().enumerate() {
            if key(Modifiers::NONE, k) {
                self.visible[i] = !self.visible[i];
            }
        }
        // Tab presses taken by `raw_input_hook` (cursors on).
        for _ in 0..std::mem::take(&mut self.tab_presses) {
            self.cursors.select_next();
        }
        if self.cursors.selected.is_some() && key(Modifiers::NONE, Key::Escape) {
            self.cursors.selected = None;
        }
        // [ and ]: previous and next search event.
        if key(Modifiers::NONE, Key::OpenBracket) {
            self.search_step(-1);
        }
        if key(Modifiers::NONE, Key::CloseBracket) {
            self.search_step(1);
        }
        // + and −: zoom the time axis around the centre.
        if key(Modifiers::NONE, Key::Plus) || key(Modifiers::NONE, Key::Equals) {
            self.pending_view = Some(ViewCommand::ZoomCenter(1.0 / 1.5));
        }
        if key(Modifiers::NONE, Key::Minus) {
            self.pending_view = Some(ViewCommand::ZoomCenter(1.5));
        }
        // Arrows: move the selected cursor (1 sample or 1/100 of the screen;
        // Shift: 1/10 of the screen), or else move the view by 1/10.
        let Some(t) = self.last_transform else { return };
        let bounds = t.bounds();
        let (width, height) = (bounds.width(), bounds.height());
        let dt = self
            .waveform
            .as_ref()
            .and_then(|w| w.ch1.as_ref().or(w.ch2.as_ref()))
            .and_then(|c| c.sample_interval())
            .unwrap_or(width / 1000.0);
        for (k, sx, sy) in [
            (Key::ArrowLeft, -1.0, 0.0),
            (Key::ArrowRight, 1.0, 0.0),
            (Key::ArrowUp, 0.0, 1.0),
            (Key::ArrowDown, 0.0, -1.0),
        ] {
            for (m, big) in [(Modifiers::SHIFT, true), (Modifiers::NONE, false)] {
                if !key(m, k) {
                    continue;
                }
                let step_t = if big { width / 10.0 } else { dt };
                let step_v = if big { height / 10.0 } else { height / 100.0 };
                if !self.cursors.nudge(sx * step_t, sy * step_v) {
                    let f = if big { 0.5 } else { 0.1 };
                    self.pending_view = Some(ViewCommand::Pan {
                        x: sx * f,
                        y: sy * f,
                    });
                }
            }
        }
    }

    /// The overview strip of the zoom view: the whole record, with the part
    /// that the main plot shows as a bright window. Click or drag: move the
    /// main view there. Wheel: zoom the main view in time.
    fn overview(&mut self, ui: &mut egui::Ui, data: [&Vec<[f64; 2]>; 2]) {
        let channels = [data[0].as_slice(), data[1].as_slice()];
        let Some(full) = time_span(&channels) else {
            return;
        };
        let span_y = voltage_span_in(&channels, &full).unwrap_or(-1.0..=1.0);
        let visible = self
            .last_transform
            .as_ref()
            .map_or_else(|| full.clone(), |t| t.bounds().range_x());
        let (v0, v1) = (
            visible.start().max(*full.start()),
            visible.end().min(*full.end()),
        );
        let (y0, y1) = (*span_y.start(), *span_y.end());
        let reduced = [
            decimate(data[0], OVERVIEW_BUCKETS),
            decimate(data[1], OVERVIEW_BUCKETS),
        ];
        let response = Plot::new(("overview", self.load_count, self.y_axis))
            .height(OVERVIEW_HEIGHT)
            .show_axes([false, false])
            .show_grid(false)
            .show_x(false)
            .show_y(false)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .allow_double_click_reset(false)
            .default_x_bounds(*full.start(), *full.end())
            .default_y_bounds(y0, y1)
            .show(ui, |plot| {
                for (points, color) in reduced.into_iter().zip([palette::ch1(), palette::ch2()]) {
                    if !points.is_empty() {
                        plot.line(Line::new("", points).color(color).width(1.0));
                    }
                }
                // Darken what the main plot does not show.
                let shade = |a: f64, b: f64| {
                    egui_plot::Polygon::new("", vec![[a, y0], [b, y0], [b, y1], [a, y1]])
                        .fill_color(palette::overview_shade())
                        .stroke(egui::Stroke::NONE)
                };
                if v0 > *full.start() {
                    plot.polygon(shade(*full.start(), v0));
                }
                if v1 < *full.end() {
                    plot.polygon(shade(v1, *full.end()));
                }
                for x in [v0, v1] {
                    plot.vline(VLine::new("", x).color(palette::overview_edge()).width(1.5));
                }
                plot.vline(
                    VLine::new("", 0.0)
                        .color(palette::trigger().gamma_multiply(0.5))
                        .style(LineStyle::Dotted { spacing: 4.0 }),
                );
            });
        let r = &response.response;
        if (r.clicked() || r.dragged())
            && let Some(pos) = r.interact_pointer_pos()
        {
            let x = response.transform.value_from_position(pos).x;
            self.pending_view = Some(ViewCommand::CenterX(x));
        }
        if r.hovered() && pointer_on_background(ui.ctx()) {
            let (delta, pos) = ui.input(|i| (wheel_delta(i), i.pointer.latest_pos()));
            let amount = delta.x + delta.y;
            if amount != 0.0
                && let Some(pos) = pos
            {
                let at = response.transform.value_from_position(pos).x;
                let factor = f64::from((-amount / WHEEL_ZOOM_POINTS).exp());
                self.pending_view = Some(ViewCommand::ZoomX { factor, at });
            }
            ui.ctx().set_cursor_icon(if r.dragged() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        }
        if self.pending_view.is_some() {
            ui.ctx().request_repaint();
        }
    }

    /// The FFT source channel: the chosen one if the file has it, else the
    /// other one. `None` without a channel.
    fn fft_source(&self) -> Option<usize> {
        let w = self.waveform.as_ref()?;
        let present = [w.ch1.is_some(), w.ch2.is_some()];
        if present[self.fft.source.min(1)] {
            Some(self.fft.source.min(1))
        } else {
            present.iter().position(|&p| p)
        }
    }

    /// Contents of the FFT menu.
    fn fft_menu(&mut self, ui: &mut egui::Ui) {
        let present = self
            .waveform
            .as_ref()
            .map_or([true, true], |w| [w.ch1.is_some(), w.ch2.is_some()]);
        ui.checkbox(&mut self.fft.show, "Show FFT");
        ui.separator();
        ui.label("Source:");
        ui.horizontal(|ui| {
            for (i, (name, color)) in [("CH1", palette::ch1()), ("CH2", palette::ch2())]
                .into_iter()
                .enumerate()
            {
                if ui
                    .add_enabled(
                        present[i],
                        egui::RadioButton::new(
                            self.fft.source == i,
                            RichText::new(name).color(color),
                        ),
                    )
                    .on_disabled_hover_text(format!("{name} is off in this file"))
                    .clicked()
                {
                    self.fft.source = i;
                }
            }
        });
        ui.label("Window:");
        for w in fft::Window::ALL {
            ui.radio_value(&mut self.fft.window, w, w.label())
                .on_hover_text(w.description());
        }
        ui.label("Scale:");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.fft.db, true, "dBV")
                .on_hover_text("Decibels relative to 1 V RMS");
            ui.radio_value(&mut self.fft.db, false, "V RMS");
        });
        ui.separator();
        ui.label(
            RichText::new("The FFT uses the samples shown in the time plot.")
                .small()
                .color(palette::hint()),
        );
    }

    /// The spectrum of the source channel over the visible time span, from
    /// the cache when nothing changed: (channel, spectrum, samples used).
    fn current_spectrum(&mut self) -> Option<(usize, fft::Spectrum, usize)> {
        let source = self.fft_source()?;
        let w = self.waveform.as_ref()?;
        let ch = [&w.ch1, &w.ch2][source].as_ref()?;
        let record = ch.points.first()?[0]..=ch.points.last()?[0];
        let span = self
            .last_transform
            .as_ref()
            .map_or(record, |t| t.bounds().range_x());
        let a = ch.points.partition_point(|p| p[0] < *span.start());
        let b = ch.points.partition_point(|p| p[0] <= *span.end());
        let count = b.saturating_sub(a);
        let key: FftKey = (
            self.load_count,
            source,
            self.fft.window,
            span.start().to_bits(),
            span.end().to_bits(),
        );
        if self.fft_cache.as_ref().map(|c| c.0) != Some(key) {
            let values: Vec<f64> = ch.points[a..b.max(a)].iter().map(|p| p[1]).collect();
            let dt = ch.sample_interval().unwrap_or(0.0);
            self.fft_cache = Some((key, fft::spectrum(&values, dt, self.fft.window)));
        }
        let spectrum = self.fft_cache.as_ref()?.1.clone()?;
        Some((source, spectrum, count.min(fft::MAX_SAMPLES)))
    }

    /// The FFT panel: a header with the peak, and the spectrum plot.
    fn fft_view(&mut self, ui: &mut egui::Ui) {
        let Some((source, spectrum, count)) = self.current_spectrum() else {
            ui.label(
                RichText::new("FFT: not enough samples in the time span shown")
                    .color(palette::hint()),
            );
            return;
        };
        let color = [palette::ch1(), palette::ch2()][source];
        let db = self.fft.db;
        let level = move |v: f64| if db { fft::dbv(v) } else { v };
        let fmt_level = move |v: f64| {
            if db {
                format!("{:.1} dBV", fft::dbv(v))
            } else {
                fmt_sig(v, "V", READOUT_DIGITS, 0.0)
            }
        };
        let peak = spectrum.peak();
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(format!(
                    "FFT CH{} · {}",
                    source + 1,
                    self.fft.window.label()
                ))
                .monospace()
                .strong()
                .color(color),
            );
            if let Some((pf, pv)) = peak {
                readout(
                    ui,
                    "Peak",
                    Some(format!(
                        "{} {}",
                        fmt_sig(pf, "Hz", READOUT_DIGITS, 0.0),
                        fmt_level(pv)
                    )),
                    color,
                );
            }
            if let Some(df) = spectrum.resolution() {
                readout(ui, "Δf", Some(fmt_sig(df, "Hz", 3, 0.0)), palette::fg_dim())
                    .on_hover_text("Frequency step between the FFT bins");
            }
            readout(ui, "N", Some(count.to_string()), palette::fg_dim())
                .on_hover_text("Samples used (the time span shown)");
        });
        let points: Vec<[f64; 2]> = spectrum
            .freqs
            .iter()
            .zip(&spectrum.vrms)
            .map(|(&f, &v)| [f, level(v)])
            .collect();
        let points = decimate(&points, FFT_BUCKETS);
        // Start view: up to the 20th harmonic of the peak (the whole span up
        // to half the sample rate is often far wider than the signal).
        let nyquist = spectrum.freqs.last().copied().unwrap_or(1.0);
        let f_max = peak.map_or(nyquist, |(pf, _)| {
            (pf * 20.0).clamp(nyquist / 1000.0, nyquist)
        });
        let mut plot = Plot::new(("fft", self.load_count, source, db))
            .show_grid(true)
            .grid_color(palette::grid())
            .x_axis_label("Frequency")
            .x_axis_formatter(|mark, _| si(mark.value, "Hz"))
            .default_x_bounds(0.0, f_max)
            .allow_scroll(false)
            .label_formatter(move |hover: &HoverPosition<'_>| {
                let p = match hover {
                    HoverPosition::NearDataPoint { position, .. } => position,
                    HoverPosition::Elsewhere { position, .. } => position,
                };
                let y = if db {
                    format!("{:.1} dBV", p.y)
                } else {
                    si(p.y, "V")
                };
                Some(format!("f = {}\n{y}", si(p.x, "Hz")))
            });
        plot = if db {
            plot.y_axis_label("dBV")
                .y_axis_formatter(|mark, _| format!("{:.0}", mark.value))
        } else {
            plot.y_axis_label("V RMS")
                .y_axis_formatter(|mark, _| si(mark.value, "V"))
        };
        plot.show(ui, |plot| {
            // Mouse wheel zooms, like the time plot.
            let (delta, modifiers) = plot.ctx().input(|i| (wheel_delta(i), i.modifiers));
            let amount = delta.x + delta.y;
            if amount != 0.0 && plot.response().hovered() && pointer_on_background(plot.ctx()) {
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
            plot.line(Line::new("FFT", points).color(color).width(1.2));
            if let Some((pf, pv)) = peak {
                plot.points(
                    egui_plot::Points::new("", vec![[pf, level(pv)]])
                        .radius(4.0)
                        .color(palette::fg()),
                );
            }
        });
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
                ui.label(RichText::new(text).size(18.0).color(palette::hint()))
            });
            return;
        }

        let fit = self.pending_fit.take();
        // Near a cursor line, a drag moves the cursor instead of the plot.
        let on_plot = pointer_on_background(ui.ctx());
        // Div mode: a drag on a ground marker moves that channel up or down.
        self.handle_marker_drag(ui.ctx(), on_plot);
        let allow_pan = self.cursors.handle_pointer(
            ui.ctx(),
            if self.marker_drag.is_some() {
                None
            } else {
                self.last_transform.as_ref()
            },
            on_plot,
        ) && self.marker_drag.is_none();
        let maps = self.y_maps();
        let [data_1, data_2] = self.plot_data();
        let reference_lines = self.reference_lines();
        let math = self.math_channel();
        let math_map = self.math_map(math.as_ref());
        let math_data: Vec<[f64; 2]> = match (&math, math_map) {
            (Some(m), Some(map)) => m.points.iter().map(|p| [p[0], map.to_plot(p[1])]).collect(),
            _ => Vec::new(),
        };
        if self.zoom_view {
            self.overview(ui, [&data_1, &data_2]);
            ui.add_space(4.0);
        }
        let view_command = self.pending_view.take();
        // The plot and its axis labels fill this area (the upper part when
        // the FFT is shown).
        let mut outer = ui.available_rect_before_wrap();
        let show_fft = self.fft.show && self.fft_source().is_some();
        if show_fft {
            outer.max.y = outer.min.y + outer.height() * TIME_PLOT_SHARE;
        }
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
        if show_fft {
            plot = plot.height(outer.height());
        }
        let response = plot
            .allow_drag(allow_pan)
            .show_grid(true)
            .grid_color(palette::grid())
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
                            "MATH" => math_map,
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
                // Not through a window or menu on top of the plot (Help).
                if amount != 0.0 && pointer_on_background(plot.ctx()) {
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
                if let Some(command) = view_command {
                    let r = plot.plot_bounds().range_x();
                    let (a, b) = (*r.start(), *r.end());
                    match command {
                        ViewCommand::CenterX(c) => {
                            let half = (b - a) / 2.0;
                            plot.set_plot_bounds_x(c - half..=c + half);
                        }
                        ViewCommand::ZoomX { factor, at } => {
                            plot.set_plot_bounds_x(at - (at - a) * factor..=at + (b - at) * factor);
                        }
                        ViewCommand::ZoomCenter(factor) => {
                            let (c, half) = ((a + b) / 2.0, (b - a) / 2.0 * factor);
                            plot.set_plot_bounds_x(c - half..=c + half);
                        }
                        ViewCommand::Pan { x, y } => {
                            let r_y = plot.plot_bounds().range_y();
                            let height = r_y.end() - r_y.start();
                            plot.translate_bounds(egui::vec2(
                                (x * (b - a)) as f32,
                                (y * height) as f32,
                            ));
                        }
                    }
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
                        .color(palette::trigger().gamma_multiply(0.35))
                        .width(1.0)
                        .style(LineStyle::Dotted { spacing: 6.0 }),
                );
                // Reference waveform first, so the traces are drawn over it.
                for (name, points, color) in reference_lines.into_iter().flatten() {
                    plot.line(Line::new(name, points).color(color).width(1.0));
                }
                if !math_data.is_empty() {
                    plot.line(
                        Line::new("MATH", math_data)
                            .color(palette::math())
                            .width(1.5),
                    );
                }
                if !data_1.is_empty() {
                    plot.line(Line::new("CH1", data_1).color(palette::ch1()).width(1.5));
                }
                if !data_2.is_empty() {
                    plot.line(Line::new("CH2", data_2).color(palette::ch2()).width(1.5));
                }
            });
        self.cursors
            .update_pointer_icon(ui.ctx(), &response.transform);
        draw_trigger_marker(ui, &response.transform);
        if self.search.kind.is_some() {
            draw_search_marks(
                ui,
                &response.transform,
                &self.search.events,
                self.search.current,
            );
        }
        draw_ground_markers(ui, &response.transform, &maps);
        if on_plot {
            let pos = ui.ctx().input(|i| i.pointer.latest_pos());
            let over = pos.and_then(|p| marker_at(&response.transform, &maps, p));
            if self.marker_drag.is_some() || over.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
            if let (Some(i), None) = (over, self.marker_drag) {
                let tip = if self.y_axis == YAxis::Divisions {
                    format!("CH{}: 0 V. Drag to move the channel up or down.", i + 1)
                } else {
                    format!(
                        "CH{}: 0 V. Drag to move the channel up or down (switches Y to Div: in Volts the channels share one axis).",
                        i + 1
                    )
                };
                egui::Tooltip::always_open(
                    ui.ctx().clone(),
                    ui.layer_id(),
                    egui::Id::unique("marker_tip"),
                    egui::PopupAnchor::Pointer,
                )
                .show(|ui| ui.label(tip));
            }
        }
        // The measurements use the view of the last frame: draw once more
        // when the view changes, so that they are never one frame late.
        let old_x = self.last_transform.as_ref().map(|t| t.bounds().range_x());
        if old_x != Some(response.transform.bounds().range_x()) {
            ui.ctx().request_repaint();
        }
        self.last_transform = Some(response.transform);
        if show_fft {
            ui.add_space(4.0);
            self.fft_view(ui);
        }
    }
}

impl eframe::App for ScopeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        palette::set_light(self.light_theme);
        self.update_shot(&ctx);
        // No shortcuts while the welcome screen or the Print window is open.
        if !self.splash_open && !self.print_dialog {
            self.handle_keys(&ctx);
        }
        self.handle_dropped_files(&ctx);
        self.receive_loaded_files();
        self.update_search();
        // A file was dropped or opened: the splash screen is no longer needed.
        if self.waveform.is_some() && self.load_count == 1 && self.splash_open {
            self.splash_open = false;
        }
        self.splash(&ctx);
        self.print_window(&ctx);
        self.stats_window(&ctx);
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
            .frame(egui::Frame::central_panel(ui.style()).fill(palette::panel_bg()))
            .show(ui, |ui| self.scope_screen(ui));
    }

    /// With cursors on, Tab selects the next cursor. Take the Tab key events
    /// here, before egui uses them to move the focus between buttons.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if self.cursors.mode == CursorMode::Off || self.splash_open || self.waveform.is_none() {
            return;
        }
        raw_input.events.retain(|e| match e {
            egui::Event::Key {
                key: egui::Key::Tab,
                pressed,
                modifiers,
                ..
            } if modifiers.is_none() => {
                if *pressed {
                    self.tab_presses += 1;
                }
                false
            }
            _ => true,
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, MEASURE_KEY, &self.measure);
        eframe::set_value(storage, Y_AXIS_KEY, &self.y_axis);
        eframe::set_value(storage, HIDE_SPLASH_KEY, &self.hide_splash);
        eframe::set_value(storage, ZOOM_VIEW_KEY, &self.zoom_view);
        eframe::set_value(storage, FFT_KEY, &self.fft);
        eframe::set_value(storage, MATH_KEY, &self.math);
        eframe::set_value(storage, LIGHT_THEME_KEY, &self.light_theme);
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
        None => ("OFF".to_owned(), palette::off(), egui::Stroke::NONE),
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
            palette::trigger(),
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
        palette::trigger(),
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
/// its trace. A hidden channel is grey.
fn channel_button(
    ui: &mut egui::Ui,
    name: &str,
    text: &str,
    color: Color32,
    shown: bool,
) -> egui::Response {
    let (shown_color, tip) = if shown {
        (color, "Click to hide this channel")
    } else {
        (palette::off(), "Click to show this channel")
    };
    let label = RichText::new(format!("{name} {text}"))
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
        ui.label(RichText::new(title).strong().color(palette::fg()));
        for line in lines {
            ui.label(format!("• {line}"));
        }
    };
    ui.label(
        RichText::new(format!("FNIRSI Viewer {VERSION} · commit {GIT_HASH}"))
            .small()
            .color(palette::hint()),
    );
    section(
        ui,
        "Open a file",
        &[
            "Drop a .wav file on the window, or use ⏏ File ▾ → Open file….",
            "File ▾ → Open example: real 1013D captures to try the viewer without a scope.",
            "Supported: DPOX180H, and 1013D/1014D with the original FNIRSI firmware. The model is found automatically and shown at the top left.",
            "The file is read on this device; nothing is uploaded.",
            "If a file cannot be opened, a red banner says why, and the file shown before stays.",
        ],
    );
    section(
        ui,
        "Compare, export and print (⏏ File menu)",
        &[
            "Open reference…: a second file drawn faint behind the traces (grey for CH1, blue-grey for CH2), to compare before and after. Click the REF readout at the bottom to hide or show it.",
            "Export CSV…: all the samples as a table (time and the voltage of each channel), with the settings in # lines at the top.",
            "Save screenshot (PNG)…: a picture of the screen as shown.",
            "Print… (Ctrl/⌘+P): choose a white background (recommended: much less ink, darker colours made for paper) or black; the screen changes only for the picture. On the desktop, the picture opens in the image viewer, where you can print it.",
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
            "⊕ Zoom: an overview of the whole record above the plot. The bright window is the part shown below. Click or drag in it to move there; wheel on it to zoom in time.",
            "⬌ Fit H: whole record. ⬍ Fit V: vertical axis fitted to what is on screen now. ⛶ Fit both: both.",
        ],
    );
    section(
        ui,
        "Vertical axis (Y: Volts / Div)",
        &[
            "Volts: one voltage axis for both channels.",
            "Div: like the scope screen, 8 divisions. Each channel uses its own V/div and starts from its ground marker.",
            "1▶ and 2▶ on the left edge: the 0 V level (ground) of each channel. Drag a marker to move that channel up or down. This needs Y: Div (each channel has its own scale); a drag in Volts mode switches to Div. Y ▾ → Reset channel positions puts them back.",
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
        "Spectrum (∿ FFT menu)",
        &[
            "Show FFT: the spectrum of one channel under the time plot, of the samples shown in the time plot (zoom in to analyse a part).",
            "Window: Hanning for general use; Rectangular for transients; Blackman-Harris for small harmonics; Flat top for accurate levels.",
            "Scale: dBV (0 dBV = 1 V RMS) or V RMS. A sine of amplitude A reads A/√2 V RMS.",
            "Peak: the highest line above DC (white dot); its frequency is interpolated between bins. Δf: the frequency step; a longer time span gives a finer step.",
            "The FFT plot opens up to 20 × the peak frequency; double-click it to return there; zoom out to see up to half the sample rate.",
        ],
    );
    section(
        ui,
        "Measurements (▤ Measure menu)",
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
        "Math and search",
        &[
            "Σ Math: CH1 + CH2, CH1 − CH2 or CH2 − CH1, sample by sample (needs both channels). The MATH trace is measured like a channel; click the MATH readout to hide it.",
            "⌕ Search: rising edges, falling edges or glitches (very short pulses) in the whole record of CH1 or CH2. Marks on the bottom edge show them; Previous/Next (keys [ and ]) centre the view on each one.",
            "Statistics per cycle (▤ Measure menu): mean, min, max, σ and number of cycles for the time measurements.",
        ],
    );
    section(
        ui,
        "Keyboard",
        &[
            "Ctrl/⌘+O: open a file. Ctrl/⌘+P: print. F1: this help. B: black or white screen. [ and ]: previous / next search result.",
            "F: fit both. H: fit time. V: fit vertical.",
            "Z: zoom view. S: spectrum (FFT). Y: volts or divisions. 1, 2: hide or show CH1, CH2.",
            "+ and −: zoom the time axis. Arrows: move the view (Shift: more).",
            "C: cursor mode (off, time, volt, both). Tab: select the next cursor; then the arrows move it (1 sample or 1/100 of the screen; Shift: 1/10). Esc: unselect.",
            "Esc: close this window, the welcome screen or an error message.",
        ],
    );
}

/// Reduce `points` to about `buckets` min/max pairs, in time order, so that a
/// long record draws fast and its peaks stay visible (like a scope display).
pub fn decimate(points: &[[f64; 2]], buckets: usize) -> Vec<[f64; 2]> {
    if buckets == 0 || points.len() <= buckets * 2 {
        return points.to_vec();
    }
    let size = points.len().div_ceil(buckets);
    points
        .chunks(size)
        .flat_map(|chunk| {
            let (mut lo, mut hi) = (chunk[0], chunk[0]);
            for p in chunk {
                if p[1] < lo[1] {
                    lo = *p;
                }
                if p[1] > hi[1] {
                    hi = *p;
                }
            }
            if lo[0] <= hi[0] { [lo, hi] } else { [hi, lo] }
        })
        .collect()
}

/// True if the pointer is over the main area of the window (the plots), not
/// over a window, menu or popup on top of it. Mouse-wheel and cursor
/// actions use this, so that scrolling the Help window does not zoom the plot.
fn pointer_on_background(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.pointer.latest_pos())
        .and_then(|p| ctx.layer_id_at(p))
        .is_some_and(|layer| layer.order == egui::Order::Background)
}

/// A menu button of the toolbar, or the same button disabled.
fn toolbar_menu(
    ui: &mut egui::Ui,
    enabled: bool,
    label: String,
    tip: &str,
    config: Option<egui::containers::menu::MenuConfig>,
    contents: impl FnOnce(&mut egui::Ui),
) {
    if !enabled {
        ui.add_enabled(false, egui::Button::new(label))
            .on_disabled_hover_text(format!("{tip} (open a file first)"));
        return;
    }
    let mut button = egui::containers::menu::MenuButton::new(label);
    if let Some(config) = config {
        button = button.config(config);
    }
    let (response, _) = button.ui(ui, |ui| {
        // The toolbar does not wrap text; text in the menu does.
        ui.style_mut().wrap_mode = None;
        contents(ui);
    });
    response.on_hover_text(tip);
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

/// Where the ground markers are: (channel, y on screen, left edge x).
/// A second marker at about the same level is put beside the first.
fn marker_layout(transform: &PlotTransform, maps: &[Option<YMap>; 2]) -> Vec<(usize, f32, f32)> {
    let frame = transform.frame();
    let mut out: Vec<(usize, f32, f32)> = Vec::new();
    for (i, map) in maps.iter().enumerate() {
        let Some(map) = map else { continue };
        let y = transform.position_from_point_y(map.to_plot(0.0));
        let dx = if out.iter().any(|(_, u, _)| (u - y).abs() < 12.0) {
            14.0
        } else {
            0.0
        };
        out.push((i, y, frame.left() + dx));
    }
    out
}

/// The ground marker under `pos`, if any (only markers on screen).
fn marker_at(
    transform: &PlotTransform,
    maps: &[Option<YMap>; 2],
    pos: egui::Pos2,
) -> Option<usize> {
    let frame = transform.frame();
    marker_layout(transform, maps)
        .into_iter()
        .filter(|&(_, y, _)| y >= frame.top() && y <= frame.bottom())
        .find(|&(_, y, left)| {
            (pos.y - y).abs() <= 8.0 && pos.x >= left - 2.0 && pos.x <= left + 16.0
        })
        .map(|(i, _, _)| i)
}

/// Ground markers on the left edge of the plot, like on a scope screen: a
/// triangle with the channel number at the 0 V level of each channel.
/// Off screen, "1▲" or "1▼" at the top or bottom edge shows where it is.
fn draw_ground_markers(ui: &egui::Ui, transform: &PlotTransform, maps: &[Option<YMap>; 2]) {
    let frame = *transform.frame();
    let painter = ui.painter_at(frame.expand(1.0));
    let font = egui::FontId::monospace(10.0);
    let colors = [palette::ch1(), palette::ch2()];
    for (i, y, left) in marker_layout(transform, maps) {
        let color = colors[i];
        let label = format!("{}", i + 1);
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

/// Statistics of the time measurements over the cycles in the window:
/// mean, min, max, standard deviation and number of cycles.
fn stats_table(
    ui: &mut egui::Ui,
    name: &str,
    r: &measure::Results,
    ch: &parser::Channel,
    color: Color32,
    settings: &measure::Settings,
) -> bool {
    let rows: Vec<(Measurement, measure::Stats)> = settings
        .shown()
        .filter_map(|m| Some((m, measure::Stats::of(&r.per_cycle(m)?)?)))
        .collect();
    if rows.is_empty() {
        return false;
    }
    egui::Grid::new(("stats", name))
        .striped(true)
        .spacing([14.0, 2.0])
        .show(ui, |ui| {
            let head = |ui: &mut egui::Ui, t: &str| {
                ui.label(RichText::new(t).small().color(palette::hint()));
            };
            head(ui, &format!("{name} per cycle"));
            for t in ["mean", "min", "max", "σ", "n"] {
                head(ui, t);
            }
            ui.end_row();
            for (m, st) in rows {
                let cell = |ui: &mut egui::Ui, text: String| {
                    ui.label(RichText::new(text).monospace().color(color));
                };
                cell(ui, m.label().to_owned());
                cell(ui, format_measurement(m, Some(st.mean), ch));
                cell(ui, format_measurement(m, Some(st.min), ch));
                cell(ui, format_measurement(m, Some(st.max), ch));
                cell(ui, format_spread(m, st.std_dev, ch));
                cell(ui, st.count.to_string());
                ui.end_row();
            }
        });
    true
}

/// A standard deviation in the unit of measurement `m` (no "<" limit).
fn format_spread(m: Measurement, v: f64, ch: &parser::Channel) -> String {
    let dt = ch.sample_interval().unwrap_or(0.0).abs();
    match m.unit() {
        Unit::Second => fmt_sig(v, "s", 3, dt / 10.0),
        Unit::Hertz => fmt_sig(v, "Hz", 3, 0.0),
        Unit::Percent => format!("{v:.2} %"),
        Unit::Volt => fmt_sig(v, "V", 3, 0.0),
    }
}

/// Search results: a small mark on the bottom edge of the plot at each event
/// in view; the current event is larger, in the trigger colour.
fn draw_search_marks(
    ui: &egui::Ui,
    transform: &PlotTransform,
    events: &[f64],
    current: Option<usize>,
) {
    let frame = *transform.frame();
    let painter = ui.painter_at(frame.expand(1.0));
    let range = transform.bounds().range_x();
    let a = events.partition_point(|&t| t < *range.start());
    let b = events.partition_point(|&t| t <= *range.end());
    // Too many marks in view: draw at most 400.
    let step = ((b - a) / 400).max(1);
    let bottom = frame.bottom();
    let mark = |x: f32, size: f32, color: Color32| {
        egui::Shape::convex_polygon(
            vec![
                egui::pos2(x - size, bottom),
                egui::pos2(x + size, bottom),
                egui::pos2(x, bottom - 1.6 * size),
            ],
            color,
            egui::Stroke::NONE,
        )
    };
    for i in (a..b).step_by(step) {
        if Some(i) != current {
            painter.add(mark(
                transform.position_from_point_x(events[i]),
                4.0,
                palette::fg_dim(),
            ));
        }
    }
    if let Some(i) = current.filter(|&i| i >= a && i < b) {
        painter.add(mark(
            transform.position_from_point_x(events[i]),
            7.0,
            palette::trigger(),
        ));
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
    use super::{Axis, YMap, axis_zone, decimate, fmt_sig, si, time_span, voltage_span_in};
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
    fn decimation_keeps_peaks_and_order() {
        let pts: Vec<[f64; 2]> = (0..10_000)
            .map(|i| [i as f64, if i == 4321 { 9.0 } else { (i % 7) as f64 }])
            .collect();
        let d = decimate(&pts, 100);
        assert!(d.len() <= 200, "{}", d.len());
        assert!(d.contains(&[4321.0, 9.0]), "the spike stays");
        assert!(d.windows(2).all(|w| w[0][0] <= w[1][0]), "time order");
        assert_eq!(decimate(&pts[..50], 100).len(), 50, "short input unchanged");
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
