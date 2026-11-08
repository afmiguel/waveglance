//! Oscilloscope-style user interface.

mod actions;
mod dialogs;
mod format;
mod input;
mod menus;
mod plot;
mod readouts;

use crate::cursors::{self, CursorMode, Cursors};
use crate::fft;
use crate::formats;
use crate::measure::{self, Measurement, Unit};
use crate::model::{Acquisition, Capture, Display, Samples, Trace, TraceId};
use crate::palette;
use crate::{examples, export};
use eframe::egui::{self, Color32, RichText};
use egui_plot::{HoverPosition, Line, LineStyle, Plot, PlotPoint, PlotTransform, VLine};
use std::cell::RefCell;
use std::ops::RangeInclusive;
use std::rc::Rc;
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
/// Mouse wheel distance (points) for one V/div step on a channel readout.
const VDIV_WHEEL_STEP: f32 = 40.0;
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

/// What the Math channel depends on: the file and the Math settings.
type MathKey = (u64, MathSettings);
/// What a measurement depends on: file, trace name, time window, Math settings.
type MeasureKey = (u64, String, u64, u64, MathSettings);

/// A trace to measure: one of the file (borrowed) or the Math channel (shared).
enum TraceRef<'a> {
    File(&'a Trace),
    Math(Rc<Trace>),
}

impl std::ops::Deref for TraceRef<'_> {
    type Target = Trace;
    fn deref(&self) -> &Trace {
        match self {
            Self::File(t) => t,
            Self::Math(t) => t,
        }
    }
}

/// A file whose format is not certain, waiting for the user to choose it
/// (the "Which format is this file?" window).
struct FormatChoice {
    target: LoadTarget,
    name: String,
    bytes: Vec<u8>,
    /// The formats that may fit, the most likely first.
    candidates: Vec<(&'static dyn formats::Format, formats::Confidence)>,
    /// Why the window is open, for the user.
    reason: String,
}

/// A file read by the open dialog or by drag-and-drop: (target, name, bytes or error).
type Loaded = (LoadTarget, String, Result<Vec<u8>, String>);

/// A reference waveform line: (name, plot points, colour).
type RefLine = (String, Vec<[f64; 2]>, Color32);

/// Math operation: the sum or the difference of two traces, sample by sample.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum MathOp {
    #[default]
    Off,
    Sum,
    Diff,
}

impl MathOp {
    pub const ALL: [Self; 3] = [Self::Off, Self::Sum, Self::Diff];

    pub fn symbol(self) -> &'static str {
        match self {
            Self::Off => "",
            Self::Sum => "+",
            Self::Diff => "−",
        }
    }

    fn apply(self, a: f64, b: f64) -> f64 {
        match self {
            Self::Off => 0.0,
            Self::Sum => a + b,
            Self::Diff => a - b,
        }
    }
}

/// Math channel settings (saved): the operation and its two operands
/// (trace indices: 0 = the first trace of the file).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MathSettings {
    pub op: MathOp,
    pub a: usize,
    pub b: usize,
}

impl Default for MathSettings {
    fn default() -> Self {
        Self {
            op: MathOp::Off,
            a: 0,
            b: 1,
        }
    }
}

/// Event search (Search menu): what to look for, in which channel, the
/// times found and the event shown now.
#[derive(Debug, Clone, Default)]
struct Search {
    kind: Option<measure::EventKind>,
    /// Index of the trace searched.
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
    pub fn divisions(trace: &Trace) -> Self {
        Self {
            scale: 1.0 / trace.display.per_div,
            offset: trace.display.zero_div,
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
    /// Index of the source trace (0 = the first trace of the file).
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
    file_name: Option<String>,
    /// The file shown (any format, in the internal model).
    waveform: Option<Capture>,
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
    ground_shift: Vec<f64>,
    /// The ground marker that the pointer is dragging (trace index).
    marker_drag: Option<usize>,
    /// Div mode: V/div chosen by the user for each channel (mouse wheel on
    /// the channel readout), instead of the V/div of the file.
    vdiv_override: Vec<Option<f64>>,
    /// Mouse wheel distance not yet used for a V/div step.
    vdiv_wheel: f32,
    /// Reference waveform (file name, waveform), drawn faint behind the traces.
    reference: Option<(String, Capture)>,
    /// Show the reference waveform.
    show_reference: bool,
    /// A screenshot in progress (Save screenshot, Print).
    shot: Option<ShotJob>,
    /// The Print window (choice of white or black screen) is open.
    print_dialog: bool,
    /// Print on the white screen (the choice in the Print window).
    print_white: bool,
    /// Math channel (saved).
    math: MathSettings,
    /// Show the Math channel trace.
    math_visible: bool,
    /// Event search.
    search: Search,
    /// Screenshots arrive here (egui calls back after the frame is drawn).
    shot_tx: Sender<std::sync::Arc<egui::ColorImage>>,
    shot_rx: Receiver<std::sync::Arc<egui::ColorImage>>,
    /// The Math channel of the current file and Math settings.
    math_cache: RefCell<Option<(MathKey, Option<Rc<Trace>>)>>,
    /// Recent measurement results: (key, results).
    measure_cache: RefCell<Vec<(MeasureKey, Rc<measure::Results>)>>,
    /// Value range of the analog traces of the file (for the digital lanes
    /// in Volts mode).
    analog_span: Option<(f64, f64)>,
    /// A file waiting for the user to choose its format.
    format_choice: Option<FormatChoice>,
    /// The last file that could not be read, for the "Open as…" button of
    /// the error banner: (target, name, bytes).
    failed_file: Option<(LoadTarget, String, Vec<u8>)>,
    /// Tab presses taken from egui while cursors are on (they select the
    /// next cursor instead of moving the keyboard focus between buttons).
    tab_presses: usize,
    /// FFT panel settings.
    fft: FftSettings,
    /// The last spectrum and what it was computed from.
    fft_cache: Option<(FftKey, Option<fft::Spectrum>)>,
    /// Each trace shown on the screen (a click on the OSD readout hides or
    /// shows a channel, like the channel button of a scope). One per trace;
    /// the per-trace lists (`visible`, `ground_shift`, `vdiv_override`) have
    /// the length of `waveform.traces`.
    visible: Vec<bool>,
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
            ground_shift: Vec::new(),
            marker_drag: None,
            vdiv_override: Vec::new(),
            vdiv_wheel: 0.0,
            tab_presses: 0,
            format_choice: None,
            analog_span: None,
            math_cache: RefCell::new(None),
            measure_cache: RefCell::new(Vec::new()),
            failed_file: None,
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
            visible: Vec::new(),
            last_transform: None,
            inbox_tx,
            inbox_rx,
        }
    }

    /// Parse the bytes of a file and show the result.
    pub fn load_bytes(&mut self, name: String, bytes: &[u8]) {
        self.open_bytes(LoadTarget::Main, name, bytes);
    }

    /// Find the format of a file and read it. If the format is not certain,
    /// the "Which format is this file?" window asks the user.
    fn open_bytes(&mut self, target: LoadTarget, name: String, bytes: &[u8]) {
        match formats::decide(bytes) {
            formats::Decision::Read(format) => self.read_as(target, name, bytes, format),
            formats::Decision::Ask(candidates) => {
                let reason = if candidates.is_empty() {
                    "No known format was recognised in this file.".to_owned()
                } else {
                    "The format of this file is not certain.".to_owned()
                };
                self.format_choice = Some(FormatChoice {
                    target,
                    name,
                    bytes: bytes.to_vec(),
                    candidates,
                    reason,
                });
            }
        }
    }

    /// Read a file with `format` and show it (main file or reference). On an
    /// error, the banner offers "Open as…" to try another format.
    fn read_as(
        &mut self,
        target: LoadTarget,
        name: String,
        bytes: &[u8],
        format: &'static dyn formats::Format,
    ) {
        match format.read(bytes) {
            Ok(w) if target == LoadTarget::Reference => {
                self.reference = Some((name, w));
                self.show_reference = true;
                self.failed_file = None;
            }
            Ok(w) => {
                self.failed_file = None;
                let n = w.traces.len();
                self.visible = vec![true; n];
                self.ground_shift = vec![0.0; n];
                self.vdiv_override = vec![None; n];
                for warning in &w.warnings {
                    log::warn!("{name}: {warning}");
                }
                self.analog_span = w
                    .traces
                    .iter()
                    .filter(|t| !t.is_digital())
                    .flat_map(|t| t.points().iter().map(|p| p[1]))
                    .filter(|v| v.is_finite())
                    .fold(None, |acc: Option<(f64, f64)>, v| {
                        Some(acc.map_or((v, v), |(lo, hi)| (lo.min(v), hi.max(v))))
                    });
                self.waveform = Some(w);
                self.error = None;
                self.file_name = Some(name);
                self.load_count += 1;
                self.marker_drag = None;
                // New file: cursors at 1/4 and 3/4 of the waveform.
                let data = self.plot_data();
                let channels: Vec<&[[f64; 2]]> = data.iter().map(Vec::as_slice).collect();
                if let Some(time) = time_span(&channels)
                    && let Some(span) = voltage_span_in(&channels, &time)
                {
                    self.cursors.place_in(&time, &span);
                }
                self.last_transform = None;
            }
            Err(e) => {
                let what = if target == LoadTarget::Reference {
                    "reference "
                } else {
                    ""
                };
                self.report_open_error(
                    &format!("{what}{name}"),
                    &format!("{} ({e})", format.name()),
                );
                self.failed_file = Some((target, name, bytes.to_vec()));
            }
        }
    }

    /// The traces of the file shown (empty without a file).
    fn traces(&self) -> &[Trace] {
        self.waveform.as_ref().map_or(&[], |w| w.traces.as_slice())
    }

    /// Vertical mapping of each trace for the current Y axis mode
    /// (`None` if the trace is hidden). One entry per trace.
    pub fn y_maps(&self) -> Vec<Option<YMap>> {
        // Digital traces: one lane each, under the analog traces (lane 0 at
        // the bottom). Lane height: 1 division (Div), or 15% of the analog
        // range (Volts).
        let traces = self.traces();
        let lanes = traces.iter().filter(|t| t.is_digital()).count();
        let (lo, hi) = self.analog_span.unwrap_or((0.0, 0.0));
        let volts_lane = if hi > lo { (hi - lo) * 0.15 } else { 1.0 };
        let mut lane = 0usize;
        traces
            .iter()
            .enumerate()
            .map(|(i, t)| {
                if t.is_digital() {
                    // Top lane first: the first digital trace is the highest.
                    let k = (lanes - 1 - lane) as f64;
                    lane += 1;
                    if !self.visible.get(i).copied().unwrap_or(true) {
                        return None;
                    }
                    return Some(match self.y_axis {
                        YAxis::Divisions => YMap {
                            scale: 0.7,
                            offset: -SCREEN_DIVS_Y / 2.0 + 0.15 + k,
                        },
                        YAxis::Volts => YMap {
                            scale: volts_lane * 0.7,
                            offset: lo - (k + 1.2) * volts_lane,
                        },
                    });
                }
                if !self.visible.get(i).copied().unwrap_or(true) {
                    return None;
                }
                Some(match self.y_axis {
                    YAxis::Divisions => {
                        let mut m = YMap::divisions(t);
                        // A V/div set by the user changes the size of the
                        // trace; the ground marker stays where it is.
                        if let Some(Some(v)) = self.vdiv_override.get(i) {
                            m.scale = 1.0 / v;
                        }
                        m.offset += self.ground_shift.get(i).copied().unwrap_or(0.0);
                        m
                    }
                    YAxis::Volts => YMap::IDENTITY,
                })
            })
            .collect()
    }

    /// The plot y below which digital lanes are drawn, where the y axis has
    /// no value scale: `None` without lanes, infinity with lanes only.
    pub(super) fn lane_zone_top(&self) -> Option<f64> {
        let traces = self.traces();
        let lanes = traces.iter().filter(|t| t.is_digital()).count();
        if lanes == 0 {
            return None;
        }
        if lanes == traces.len() {
            return Some(f64::INFINITY);
        }
        match (self.y_axis, self.analog_span) {
            // Volts: the lanes are under the analog range (see `y_maps`).
            (YAxis::Volts, Some((lo, hi))) => {
                let lane = if hi > lo { (hi - lo) * 0.15 } else { 1.0 };
                Some(lo - 0.35 * lane)
            }
            // Divisions: the lanes share the screen; keep the scale.
            _ => None,
        }
    }

    /// Mappings for the ground markers: analog traces only (a digital trace
    /// has a lane, not a ground marker).
    fn marker_maps(&self) -> Vec<Option<YMap>> {
        self.y_maps()
            .into_iter()
            .zip(self.traces())
            .map(|(m, t)| m.filter(|_| !t.is_digital()))
            .collect()
    }

    /// The Math channel can be computed: an operation, and two different
    /// traces that exist in the file.
    fn math_available(&self) -> bool {
        let n = self.traces().len();
        self.math.op != MathOp::Off
            && self.math.a != self.math.b
            && self.math.a < n
            && self.math.b < n
    }

    /// Label of the Math channel: "CH1 − CH2".
    fn math_label(&self) -> String {
        let t = self.traces();
        let name = |i: usize| t.get(i).map_or("?", |t| t.label.as_str());
        format!(
            "{} {} {}",
            name(self.math.a),
            self.math.op.symbol(),
            name(self.math.b)
        )
    }

    /// The Math channel, computed sample by sample from its two traces (both
    /// in the file, with the same samples). Its V/div is the larger of the two.
    fn compute_math(&self) -> Option<Trace> {
        if !self.math_available() {
            return None;
        }
        let t = self.traces();
        let (a, b) = (t.get(self.math.a)?, t.get(self.math.b)?);
        let (pa, pb) = (a.points(), b.points());
        if pa.len() != pb.len() {
            return None;
        }
        let points = pa
            .iter()
            .zip(pb)
            .map(|(p, q)| [p[0], self.math.op.apply(p[1], q[1])])
            .collect();
        let mut trace = Trace::new(
            TraceId::Math,
            "MATH",
            Samples::Timed(points),
            Display {
                per_div: a.display.per_div.max(b.display.per_div),
                zero_div: 0.0,
                probe: 1.0,
                coupling: None,
            },
        );
        // One code: the finer of the two traces.
        trace.resolution = Some(a.lsb().min(b.lsb()));
        Some(trace)
    }

    /// The Math channel, from the cache (computed again only when the file
    /// or the Math settings change).
    pub fn math_channel(&self) -> Option<Rc<Trace>> {
        let key = (self.load_count, self.math);
        let mut cache = self.math_cache.borrow_mut();
        if cache.as_ref().map(|c| c.0) != Some(key) {
            *cache = Some((key, self.compute_math().map(Rc::new)));
        }
        cache.as_ref()?.1.clone()
    }

    /// Measurements of a trace in a time window, from a cache: the readouts
    /// and the statistics window use the same results in a frame.
    fn measure_cached(
        &self,
        name: &str,
        ch: &Trace,
        window: &RangeInclusive<f64>,
    ) -> Rc<measure::Results> {
        let key = (
            self.load_count,
            name.to_owned(),
            window.start().to_bits(),
            window.end().to_bits(),
            self.math,
        );
        let mut cache = self.measure_cache.borrow_mut();
        if let Some((_, r)) = cache.iter().find(|(k, _)| *k == key) {
            return r.clone();
        }
        let r = Rc::new(measure::measure(
            ch.points(),
            window,
            MIN_AMPLITUDE_DIV * ch.display.per_div,
        ));
        if cache.len() >= 8 {
            cache.remove(0);
        }
        cache.push((key, r.clone()));
        r
    }

    /// Plot mapping of the Math channel (`None` if off or hidden).
    fn math_map(&self, math: Option<&Trace>) -> Option<YMap> {
        let m = math.filter(|_| self.math_visible)?;
        Some(match self.y_axis {
            YAxis::Divisions => YMap {
                scale: 1.0 / m.display.per_div,
                offset: 0.0,
            },
            YAxis::Volts => YMap::IDENTITY,
        })
    }

    /// The channel points in plot units: `[time_s, y]`.
    /// One entry per trace (empty if hidden).
    fn plot_data(&self) -> Vec<Vec<[f64; 2]>> {
        self.plot_data_in(None, usize::MAX)
    }

    /// The trace points to draw: only the samples in `view` (all if `None`),
    /// reduced to about `max_points` min/max points, in plot units. When
    /// cut to the view, the first and last samples of the record are kept, so
    /// that the automatic bounds (double-click reset) still cover the whole
    /// record. One entry per trace (empty if hidden).
    fn plot_data_in(
        &self,
        view: Option<&RangeInclusive<f64>>,
        max_points: usize,
    ) -> Vec<Vec<[f64; 2]>> {
        self.traces()
            .iter()
            .zip(self.y_maps())
            .map(|(t, map)| {
                let Some(m) = map else { return Vec::new() };
                let all = t.points();
                let part = match view {
                    Some(v) => {
                        // One sample of margin on each side, so the line
                        // reaches the edges of the plot.
                        let a = all.partition_point(|p| p[0] < *v.start()).saturating_sub(1);
                        let b = (all.partition_point(|p| p[0] <= *v.end()) + 1).min(all.len());
                        &all[a..b.max(a)]
                    }
                    None => all,
                };
                let mut out = decimate(part, max_points / 2);
                if view.is_some() && part.len() < all.len() {
                    if let Some(first) = all.first().filter(|f| out.first() != Some(f)) {
                        out.insert(0, *first);
                    }
                    if let Some(last) = all.last().filter(|l| out.last() != Some(l)) {
                        out.push(*last);
                    }
                }
                for p in &mut out {
                    p[1] = m.to_plot(p[1]);
                }
                out
            })
            .collect()
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

    /// A file could not be opened. The file shown before (if any) stays, and
    /// the message says so, so that its data is not taken for the new file.
    fn report_open_error(&mut self, name: &str, error: &str) {
        let still = match &self.file_name {
            Some(shown) if self.waveform.is_some() => format!(" Still showing {shown}."),
            _ => String::new(),
        };
        self.error = Some(format!("Could not open {name}: {error}.{still}"));
    }

    /// Open a reference file (same detection as the main file).
    fn load_reference(&mut self, name: String, bytes: &[u8]) {
        self.open_bytes(LoadTarget::Reference, name, bytes);
    }
}

impl eframe::App for ScopeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        palette::set_light(self.light_theme);
        self.update_shot(&ctx);
        // No shortcuts while the welcome screen or a window that asks is open.
        if !self.splash_open && !self.print_dialog && self.format_choice.is_none() {
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
        self.format_window(&ctx);
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

// Helpers of the submodules, for the parent, the other submodules and the tests.
use self::{format::*, plot::*, readouts::*};

#[cfg(test)]
mod tests {
    use super::{
        Axis, YMap, axis_zone, decimate, fmt_sig, si, time_span, vdiv_step, voltage_span_in,
    };
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
    fn vdiv_steps() {
        assert_eq!(vdiv_step(1.0, true), 0.5);
        assert_eq!(vdiv_step(0.5, true), 0.2);
        assert_eq!(vdiv_step(0.2, false), 0.5);
        assert_eq!(
            vdiv_step(2.5, true),
            2.0,
            "1013D 2.5 V/div: to the next step"
        );
        assert_eq!(vdiv_step(2.5, false), 5.0);
        assert_eq!(vdiv_step(0.001, true), 0.001, "lowest step");
        assert_eq!(vdiv_step(500.0, false), 500.0, "highest step");
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
