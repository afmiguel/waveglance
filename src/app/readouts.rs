//! The readouts under the plot: OSD, measurements and cursor values.

use super::*;
use crate::model::Unit as TraceUnit;

impl ScopeApp {
    /// On-screen display: the digital readouts of the scope.
    pub(super) fn osd(&mut self, ui: &mut egui::Ui) {
        // V/div, probe factor and, if the file stores it, the input coupling.
        let channels: Vec<(String, f64, String, TraceUnit)> = self
            .traces()
            .iter()
            .map(|t| {
                let coupling = t
                    .display
                    .coupling
                    .map_or(String::new(), |k| format!(" {}", k.name()));
                (
                    t.label.clone(),
                    t.display.per_div,
                    format!(" {}X{coupling}", t.display.probe),
                    t.unit,
                )
            })
            .collect();
        let time_per_div = self.waveform.as_ref().and_then(|w| w.time_per_div);
        ui.horizontal_wrapped(|ui| {
            // Channel readouts are buttons: a click hides or shows the trace.
            for (i, (name, vdiv, probe, unit)) in channels.iter().enumerate() {
                let (name, vdiv) = (name.as_str(), *vdiv);
                let color = palette::channel(i);
                // A digital trace: no V/div; a click hides or shows it.
                if *unit == TraceUnit::Logic {
                    if channel_button(ui, name, "logic", color, self.visible[i])
                        .on_hover_text("Digital signal (0/1). Click to hide or show it.")
                        .clicked()
                    {
                        self.visible[i] = !self.visible[i];
                    }
                    continue;
                }
                let unit = unit.symbol();
                // In Div mode a V/div chosen with the wheel replaces the file
                // value; a "*" marks it.
                let shown = match (self.y_axis, self.vdiv_override[i]) {
                    (YAxis::Divisions, Some(v)) => v,
                    _ => vdiv,
                };
                let star = if shown == vdiv { "" } else { "*" };
                let text = format!("{}/div{star}{probe}", si(shown, unit));
                let response = channel_button(ui, name, &text, color, self.visible[i]).on_hover_text(
                    format!(
                        "Mouse wheel: {unit}/div of {name} (1-2-5 steps; switches Y to Div). In the file: {}/div.",
                        si(vdiv, unit)
                    ),
                );
                if response.clicked() {
                    self.visible[i] = !self.visible[i];
                }
                // Mouse wheel on the readout: the V/div knob of the channel.
                if response.hovered() {
                    let amount = ui.input(|inp| {
                        let d = wheel_delta(inp);
                        d.x + d.y
                    });
                    self.vdiv_wheel += amount;
                    if self.vdiv_wheel.abs() >= VDIV_WHEEL_STEP {
                        // Wheel up: smaller V/div (a larger trace), like turning the knob right.
                        let finer = self.vdiv_wheel > 0.0;
                        self.vdiv_wheel = 0.0;
                        if self.y_axis != YAxis::Divisions {
                            self.set_y_axis(YAxis::Divisions);
                        }
                        let next = vdiv_step(shown, finer);
                        self.vdiv_override[i] = if (next - vdiv).abs() < vdiv * 1e-9 { None } else { Some(next) };
                    }
                }
            }
            if let Some(t) = time_per_div {
                readout(ui, "H", Some(format!("{}/div", si(t, "s"))), palette::fg());
            }

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
                    Some(format!("{} {edge} {}", t.mode, t.source.as_deref().unwrap_or("?"))),
                    palette::trigger(),
                );
            }
            readout(ui, "PTS", Some(w.record_length.to_string()), palette::fg_dim());
            // Math channel: a click hides or shows it.
            if let Some(m) = self.math_channel() {
                let text = format!("{} {}/div", self.math_label(), si(m.display.per_div, "V"));
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
                    Some(format!(
                        "{} {} {at}/{n}",
                        kind.label(),
                        w.traces.get(self.search.source).map_or("", |t| t.label.as_str())
                    )),
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

            let clipped: Vec<&str> = w
                .traces
                .iter()
                .filter(|t| t.clipped())
                .map(|t| t.label.as_str())
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

    /// The time window of the measurements, and its name for the readout.
    pub(super) fn measure_window(&self) -> Option<(RangeInclusive<f64>, &'static str)> {
        let record = self.waveform.as_ref()?.time_span()?;
        let screen = || {
            self.last_transform.as_ref().map_or_else(
                || (record.clone(), "Record"),
                |t| (t.bounds().range_x(), "Screen"),
            )
        };
        Some(match self.measure.window {
            measure::Window::Record => (record.clone(), "Record"),
            measure::Window::Screen => screen(),
            measure::Window::Cursors if self.cursors.mode.shows_time() => {
                let [a, b] = self.cursors.t;
                (a.min(b)..=a.max(b), "T1–T2")
            }
            measure::Window::Cursors => screen(),
        })
    }

    /// The traces to measure: (name, trace, colour, clipped in the window).
    /// Each trace that is shown and ticked in the Measure menu; MATH when on
    /// and shown.
    pub(super) fn measured_channels(
        &self,
        window: &RangeInclusive<f64>,
    ) -> Vec<(String, TraceRef<'_>, Color32, bool)> {
        let mut rows = Vec::new();
        let traces = self.traces();
        for (i, t) in traces.iter().enumerate() {
            if self.measure.is_measured(&t.label) && self.visible[i] {
                rows.push((
                    t.label.clone(),
                    TraceRef::File(t),
                    palette::channel(i),
                    t.clipped_in(window),
                ));
            }
        }
        if let Some(m) = self.math_channel().filter(|_| self.math_visible) {
            // Clipped if a source trace is clipped in the window.
            let clipped = [self.math.a, self.math.b]
                .iter()
                .any(|&i| traces.get(i).is_some_and(|t| t.clipped_in(window)));
            rows.push((
                "MATH".to_owned(),
                TraceRef::Math(m),
                palette::math(),
                clipped,
            ));
        }
        rows
    }

    /// Automatic measurement readouts, one row per channel.
    pub(super) fn measure_readout(&self, ui: &mut egui::Ui) {
        let Some((window, window_name)) = self.measure_window() else {
            return;
        };
        ui.label(
            RichText::new(format!("Measurements · {window_name}"))
                .small()
                .color(palette::hint()),
        )
        .on_hover_text("The samples measured: Measure ▾ → Samples");
        for (name, ch, color, clipped) in self.measured_channels(&window) {
            let r = self.measure_cached(&name, &ch, &window);
            let ch = &*ch;
            ui.horizontal_wrapped(|ui| {
                tag(ui, &name, color);
                // Voltage measurements mean nothing on a digital signal.
                for m in self
                    .measure
                    .shown()
                    // Nor do rise and fall times: the steps are ideal.
                    .filter(|m| !ch.is_digital() || m.applies_to_logic())
                {
                    let value = format_measurement(m, r.get(m), ch);
                    // A clipped signal hides its real peaks: voltage values
                    // are not reliable (the real Vpp is larger).
                    if clipped && m.unit() == Unit::Volt {
                        item(ui, m.label(), &format!("?{value}"), palette::warn()).on_hover_text(
                            format!("{}\n⚠ {name} is clipped in this window: the real value can be larger", m.description()),
                        );
                    } else {
                        item(ui, m.label(), &value, color).on_hover_text(m.description());
                    }
                }
            });
        }
    }

    /// Show the measurement panel?
    pub(super) fn shows_measurements(&self) -> bool {
        let any_trace = self
            .traces()
            .iter()
            .enumerate()
            .any(|(i, t)| self.visible[i] && self.measure.is_measured(&t.label));
        self.waveform.is_some()
            && !self.measure.selected.is_empty()
            && (any_trace || (self.math_available() && self.math_visible))
    }

    /// Cursor readouts: positions, differences and channel values at T1/T2.
    pub(super) fn cursor_readout(&self, ui: &mut egui::Ui) {
        let c = &self.cursors;
        if c.mode.shows_time() {
            ui.horizontal_wrapped(|ui| {
                let color = palette::time_cursor();
                let fmt_t = |t| fmt_sig(t, "s", READOUT_DIGITS, 0.0);
                item(ui, "T1", &fmt_t(c.t[0]), color);
                item(ui, "T2", &fmt_t(c.t[1]), color);
                item(ui, "ΔT", &fmt_t(c.delta_t()), color);
                item(
                    ui,
                    "1/ΔT",
                    &c.frequency()
                        .map_or("—".to_owned(), |f| fmt_sig(f, "Hz", READOUT_DIGITS, 0.0)),
                    color,
                );
                for (i, ch) in self.traces().iter().enumerate() {
                    if !self.visible[i] {
                        continue;
                    }
                    let (name, color) = (ch.label.as_str(), palette::channel(i));
                    let at = |t| cursors::value_at(ch.points(), t);
                    let lsb = ch.lsb();
                    let fmt = |v: Option<f64>| {
                        v.map_or("—".to_owned(), |v| {
                            fmt_sig(v, ch.unit.symbol(), READOUT_DIGITS, lsb)
                        })
                    };
                    let (a, b) = (at(c.t[0]), at(c.t[1]));
                    let delta = a.zip(b).map(|(a, b)| b - a);
                    tag(ui, name, color);
                    item(ui, "@T1", &fmt(a), color);
                    item(ui, "@T2", &fmt(b), color);
                    item(ui, "Δ", &fmt(delta), color);
                }
            });
        }
        if c.mode.shows_volt() {
            ui.horizontal_wrapped(|ui| {
                let color = palette::volt_cursor();
                let fmt_v = |v| fmt_sig(v, "V", READOUT_DIGITS, 0.0);
                if self.y_axis == YAxis::Volts {
                    item(ui, "V1", &fmt_v(c.v[0]), color);
                    item(ui, "V2", &fmt_v(c.v[1]), color);
                    item(ui, "ΔV", &fmt_v(c.delta_v()), color);
                    return;
                }
                // Divisions: the cursor lines are in divisions; each channel
                // reads them in its own volts.
                ui.label(RichText::new("V1/V2").monospace().strong().color(color));
                let maps = self.y_maps();
                for (i, t) in self.traces().iter().enumerate() {
                    let (name, ch_color) = (t.label.as_str(), palette::channel(i));
                    let Some(m) = maps[i] else { continue };
                    let (a, b) = (m.to_volts(c.v[0]), m.to_volts(c.v[1]));
                    tag(ui, name, ch_color);
                    item(ui, "V1", &fmt_v(a), ch_color);
                    item(ui, "V2", &fmt_v(b), ch_color);
                    item(ui, "ΔV", &fmt_v(b - a), ch_color);
                }
            });
        }
    }
}

/// A channel name as a filled tag (dark text on the channel colour), like
/// the channel labels of a scope.
pub(super) fn tag(ui: &mut egui::Ui, name: &str, color: Color32) -> egui::Response {
    ui.add(
        egui::Label::new(
            RichText::new(format!(" {name} "))
                .monospace()
                .strong()
                .color(palette::screen_bg())
                .background_color(color),
        )
        .wrap_mode(egui::TextWrapMode::Extend),
    )
}

/// A measured value: a dim label and the value in the colour of its trace,
/// without a box (the boxes are for the settings in the OSD).
pub(super) fn item(ui: &mut egui::Ui, label: &str, value: &str, color: Color32) -> egui::Response {
    let font = egui::FontId::monospace(egui::TextStyle::Monospace.resolve(ui.style()).size);
    let mut job = egui::text::LayoutJob::default();
    job.append(
        label,
        0.0,
        egui::TextFormat::simple(font.clone(), palette::fg_dim()),
    );
    job.append(value, 6.0, egui::TextFormat::simple(font, color));
    // One piece: a wrapped row moves the whole item to the next line.
    let r = ui.add(
        egui::Label::new(job)
            .wrap_mode(egui::TextWrapMode::Extend)
            .sense(egui::Sense::hover()),
    );
    ui.add_space(8.0);
    r
}

/// One OSD readout: a coloured label and its value.
pub(super) fn readout(
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

/// The OSD readout of a channel that is on, as a button that hides or shows
/// its trace. A hidden channel is grey.
pub(super) fn channel_button(
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
    // The name as a filled tag, then the setting.
    let font = egui::FontId::monospace(egui::TextStyle::Monospace.resolve(ui.style()).size);
    let mut label = egui::text::LayoutJob::default();
    label.append(
        &format!(" {name} "),
        0.0,
        egui::TextFormat {
            background: shown_color,
            ..egui::TextFormat::simple(font.clone(), palette::screen_bg())
        },
    );
    label.append(text, 6.0, egui::TextFormat::simple(font, shown_color));
    ui.add(
        egui::Button::new(label)
            .fill(Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.0, shown_color))
            .corner_radius(3),
    )
    .on_hover_text(tip)
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Statistics of the time measurements over the cycles in the window:
/// mean, min, max, standard deviation and number of cycles.
pub(super) fn stats_table(
    ui: &mut egui::Ui,
    name: &str,
    r: &measure::Results,
    ch: &Trace,
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
pub(super) fn format_spread(m: Measurement, v: f64, ch: &Trace) -> String {
    let dt = ch.time_step().unwrap_or(0.0);
    match m.unit() {
        Unit::Second => fmt_sig(v, "s", 3, dt / 10.0),
        Unit::Hertz => fmt_sig(v, "Hz", 3, 0.0),
        Unit::Percent => format!("{v:.2} %"),
        Unit::Volt => fmt_sig(v, ch.unit.symbol(), 3, 0.0),
    }
}

/// Text of one measurement of a channel, with fixed significant digits and
/// no more resolution than the samples have.
pub(super) fn format_measurement(m: Measurement, value: Option<f64>, ch: &Trace) -> String {
    let Some(v) = value else {
        return "—".to_owned();
    };
    let dt = ch.time_step().unwrap_or(0.0);
    match m.unit() {
        // Mean and RMS average many samples, so they resolve less than one code.
        Unit::Volt => {
            let lsb = ch.lsb();
            let res = if matches!(m, Measurement::Mean | Measurement::Rms) {
                lsb / 10.0
            } else {
                lsb
            };
            fmt_sig(v, ch.unit.symbol(), READOUT_DIGITS, res)
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
