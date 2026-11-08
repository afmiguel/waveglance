//! The time plot, the overview strip, the FFT plot and what is drawn on them.

use super::*;

impl ScopeApp {
    /// The reference waveform as plot lines (name, points, colour): mapped
    /// like the main traces (Volts: the same axis; Div: its own V/div and
    /// ground), and reduced for drawing.
    pub(super) fn reference_lines(&self) -> Vec<RefLine> {
        let Some((_, w)) = self.reference.as_ref().filter(|_| self.show_reference) else {
            return Vec::new();
        };
        // Only inside the time span of the main file, so that the reference
        // never changes the view (auto bounds, Fit, reset).
        let Some(span) = self.waveform.as_ref().and_then(Capture::time_span) else {
            return Vec::new();
        };
        w.traces
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let map = match self.y_axis {
                    YAxis::Divisions => YMap::divisions(t),
                    YAxis::Volts => YMap::IDENTITY,
                };
                let points: Vec<[f64; 2]> = t
                    .points()
                    .iter()
                    .filter(|p| span.contains(&p[0]))
                    .map(|p| [p[0], map.to_plot(p[1])])
                    .collect();
                (
                    format!("REF {}", t.label),
                    decimate(&points, 4 * OVERVIEW_BUCKETS),
                    palette::reference(i),
                )
            })
            .collect()
    }

    /// The overview strip of the zoom view: the whole record, with the part
    /// that the main plot shows as a bright window. Click or drag: move the
    /// main view there. Wheel: zoom the main view in time.
    pub(super) fn overview(&mut self, ui: &mut egui::Ui, data: &[Vec<[f64; 2]>]) {
        let digital: Vec<bool> = self.traces().iter().map(Trace::is_digital).collect();
        let channels: Vec<&[[f64; 2]]> = data.iter().map(Vec::as_slice).collect();
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
        let reduced: Vec<Vec<[f64; 2]>> =
            data.iter().map(|d| decimate(d, OVERVIEW_BUCKETS)).collect();
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
                for (i, points) in reduced.into_iter().enumerate() {
                    let color = palette::channel(i);
                    if !points.is_empty() {
                        plot.line(
                            Line::new("", points)
                                .color(color)
                                .width(1.0)
                                .allow_band(!digital.get(i).copied().unwrap_or(false)),
                        );
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

    /// The FFT source trace: the chosen one if the file has it, else the
    /// first one. `None` without a trace.
    pub(super) fn fft_source(&self) -> Option<usize> {
        let n = self.traces().len();
        if n == 0 {
            None
        } else if self.fft.source < n {
            Some(self.fft.source)
        } else {
            Some(0)
        }
    }

    /// The spectrum of the source channel over the visible time span, from
    /// the cache when nothing changed: (channel, spectrum, samples used).
    pub(super) fn current_spectrum(&mut self) -> Option<(usize, fft::Spectrum, usize)> {
        let source = self.fft_source()?;
        let ch = self.traces().get(source)?;
        let points = ch.points();
        let record = points.first()?[0]..=points.last()?[0];
        let span = self
            .last_transform
            .as_ref()
            .map_or(record, |t| t.bounds().range_x());
        let a = points.partition_point(|p| p[0] < *span.start());
        let b = points.partition_point(|p| p[0] <= *span.end());
        let count = b.saturating_sub(a);
        let key: FftKey = (
            self.load_count,
            source,
            self.fft.window,
            span.start().to_bits(),
            span.end().to_bits(),
        );
        if self.fft_cache.as_ref().map(|c| c.0) != Some(key) {
            let values: Vec<f64> = points[a..b.max(a)].iter().map(|p| p[1]).collect();
            let dt = ch.sample_interval().unwrap_or(0.0);
            self.fft_cache = Some((key, fft::spectrum(&values, dt, self.fft.window)));
        }
        let spectrum = self.fft_cache.as_ref()?.1.clone()?;
        Some((source, spectrum, count.min(fft::MAX_SAMPLES)))
    }

    /// The FFT panel: a header with the peak, and the spectrum plot.
    pub(super) fn fft_view(&mut self, ui: &mut egui::Ui) {
        let Some((source, spectrum, count)) = self.current_spectrum() else {
            ui.label(
                RichText::new("FFT: not enough samples in the time span shown")
                    .color(palette::hint()),
            );
            return;
        };
        let color = palette::channel(source);
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

    pub(super) fn scope_screen(&mut self, ui: &mut egui::Ui) {
        let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        if self.waveform.is_none() || hovering {
            let text = if hovering {
                "Release to load the file"
            } else {
                "Drop a waveform file here (FNIRSI .wav, Keysight .bin, Rigol .bin/.wfm, VCD, or CSV), or use File ▾ → Open file…"
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
        // Only the part of each trace in view, about 2 points per pixel: a
        // long record draws as fast as a short one.
        let view = self.last_transform.as_ref().map(|t| t.bounds().range_x());
        let max_points = (ui.available_width().max(200.0) * 2.0) as usize;
        let data = self.plot_data_in(view.as_ref(), max_points);
        let record_span = self.waveform.as_ref().and_then(Capture::time_span);
        let labels: Vec<String> = self.traces().iter().map(|t| t.label.clone()).collect();
        let digital: Vec<bool> = self.traces().iter().map(Trace::is_digital).collect();
        let reference_lines = self.reference_lines();
        let math = self.math_channel();
        let math_map = self.math_map(math.as_deref());
        let math_data: Vec<[f64; 2]> = match (&math, math_map) {
            (Some(m), Some(map)) => m
                .points()
                .iter()
                .map(|p| [p[0], map.to_plot(p[1])])
                .collect(),
            _ => Vec::new(),
        };
        if self.zoom_view {
            let whole = self.plot_data_in(None, 2 * OVERVIEW_BUCKETS);
            self.overview(ui, &whole);
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
            .label_formatter({
                let maps = maps.clone();
                let labels = labels.clone();
                let units: Vec<&'static str> =
                    self.traces().iter().map(|t| t.unit.symbol()).collect();
                move |hover: &HoverPosition<'_>| {
                    let t = |p: &PlotPoint| format!("t = {}", si(p.x, "s"));
                    Some(match hover {
                        // On a trace: the value of that channel, in its volts.
                        HoverPosition::NearDataPoint {
                            plot_name,
                            position,
                            ..
                        } => {
                            let index = labels.iter().position(|l| l == plot_name);
                            let (map, unit) = if *plot_name == "MATH" {
                                (math_map, "V")
                            } else {
                                (index.and_then(|i| maps[i]), index.map_or("V", |i| units[i]))
                            };
                            match map {
                                // Digital: the logic level.
                                Some(m) if unit.is_empty() => format!(
                                    "{plot_name}\n{}\n= {}",
                                    t(position),
                                    match m.to_volts(position.y) {
                                        v if v < 0.25 => "0",
                                        v if v > 0.75 => "1",
                                        _ => "x / z",
                                    }
                                ),
                                Some(m) => format!(
                                    "{plot_name}\n{}\nv = {}",
                                    t(position),
                                    si(m.to_volts(position.y), unit)
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
                }
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
                    let channels: Vec<&[[f64; 2]]> = data.iter().map(Vec::as_slice).collect();
                    let mut time = plot.plot_bounds().range_x();
                    if matches!(fit, Fit::Horizontal | Fit::Both)
                        && let Some(full) = record_span.clone()
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
                for (name, points, color) in reference_lines {
                    plot.line(Line::new(name, points).color(color).width(1.0));
                }
                if !math_data.is_empty() {
                    plot.line(
                        Line::new("MATH", math_data)
                            .color(palette::math())
                            .width(1.5),
                    );
                }
                for (i, (points, label)) in data.into_iter().zip(labels).enumerate() {
                    if !points.is_empty() {
                        // Digital steps have two points at each edge time:
                        // egui_plot would merge them into filled bands.
                        plot.line(
                            Line::new(label, points)
                                .color(palette::channel(i))
                                .width(1.5)
                                .allow_band(!digital[i]),
                        );
                    }
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
        let marker_maps = self.marker_maps();
        draw_ground_markers(ui, &response.transform, &marker_maps);
        draw_lane_labels(ui, &response.transform, self.traces(), &maps);
        if on_plot {
            let pos = ui.ctx().input(|i| i.pointer.latest_pos());
            let over = pos.and_then(|p| marker_at(&response.transform, &marker_maps, p));
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

/// Mouse-wheel movement in this frame, in screen points, from the raw wheel
/// events. Unlike `smooth_scroll_delta`, nothing carries over to the next
/// frames, so a zoom or pan stops as soon as the wheel stops and never
/// reaches another area that the pointer moves to.
pub(super) fn wheel_delta(input: &egui::InputState) -> egui::Vec2 {
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

/// The trigger marker: a "T" in a small triangle on the top edge of the plot,
/// at t = 0, like on a scope screen. Off screen, an arrow at the edge points to it.
pub(super) fn draw_trigger_marker(ui: &egui::Ui, transform: &PlotTransform) {
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

/// True if the pointer is over the main area of the window (the plots), not
/// over a window, menu or popup on top of it. Mouse-wheel and cursor
/// actions use this, so that scrolling the Help window does not zoom the plot.
pub(super) fn pointer_on_background(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.pointer.latest_pos())
        .and_then(|p| ctx.layer_id_at(p))
        .is_some_and(|layer| layer.order == egui::Order::Background)
}

/// Where the ground markers are: (channel, y on screen, left edge x).
/// A second marker at about the same level is put beside the first.
pub(super) fn marker_layout(
    transform: &PlotTransform,
    maps: &[Option<YMap>],
) -> Vec<(usize, f32, f32)> {
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
pub(super) fn marker_at(
    transform: &PlotTransform,
    maps: &[Option<YMap>],
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
pub(super) fn draw_ground_markers(ui: &egui::Ui, transform: &PlotTransform, maps: &[Option<YMap>]) {
    let frame = *transform.frame();
    let painter = ui.painter_at(frame.expand(1.0));
    let font = egui::FontId::monospace(10.0);
    for (i, y, left) in marker_layout(transform, maps) {
        let color = palette::channel(i);
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

/// Search results: a small mark on the bottom edge of the plot at each event
/// in view; the current event is larger, in the trigger colour.
pub(super) fn draw_search_marks(
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

/// The name of each digital trace at the left of its lane, in its colour.
pub(super) fn draw_lane_labels(
    ui: &egui::Ui,
    transform: &PlotTransform,
    traces: &[Trace],
    maps: &[Option<YMap>],
) {
    let frame = *transform.frame();
    let painter = ui.painter_at(frame);
    for (i, (t, map)) in traces.iter().zip(maps).enumerate() {
        let Some(map) = map.filter(|_| t.is_digital()) else {
            continue;
        };
        let y = transform.position_from_point_y(map.to_plot(1.0)) - 2.0;
        painter.text(
            egui::pos2(frame.left() + 4.0, y),
            egui::Align2::LEFT_BOTTOM,
            &t.label,
            egui::FontId::monospace(10.0),
            palette::channel(i),
        );
    }
}
