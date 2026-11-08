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
        let buses: Vec<bool> = self.traces().iter().map(Trace::is_bus).collect();
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
                    // A bus has no level to draw at this size.
                    if !points.is_empty() && !buses.get(i).copied().unwrap_or(false) {
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
            let (delta, pos, modifiers) =
                ui.input(|i| (wheel_delta(i), i.pointer.latest_pos(), i.modifiers));
            let amount = zoom_amount(delta, modifiers);
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

    /// The value of the bus under the pointer, for the hover label:
    /// "data = 3F (hex) = 63". `None` if the pointer is not on a bus lane.
    fn bus_hover(&self, ctx: &egui::Context, maps: &[Option<YMap>]) -> Option<String> {
        let transform = self.last_transform.as_ref()?;
        let pos = ctx.input(|i| i.pointer.latest_pos())?;
        let at = transform.value_from_position(pos);
        self.traces().iter().zip(maps).find_map(|(t, map)| {
            let (bits, map) = (t.bus?, (*map)?);
            let (lo, hi) = (map.to_plot(0.0), map.to_plot(1.0));
            if !(lo.min(hi)..=lo.max(hi)).contains(&at.y) {
                return None;
            }
            let v = cursors::value_at(t.points(), at.x)?;
            let mut text = format!(
                "{} = {} ({})",
                t.label,
                bus_text(v, bits, self.bus_radix),
                self.bus_radix.label().to_lowercase()
            );
            if self.bus_radix != Radix::Dec && v.is_finite() {
                text.push_str(&format!(" = {}", bus_text(v, bits, Radix::Dec)));
            }
            Some(text)
        })
    }

    /// The FFT source trace: the chosen one if the file has it, else the
    /// first one. `None` without a trace.
    pub(super) fn fft_source(&self) -> Option<usize> {
        // A bus has values, not a waveform: no spectrum.
        let t = self.traces();
        if t.get(self.fft.source).is_some_and(|t| !t.is_bus()) {
            Some(self.fft.source)
        } else {
            t.iter().position(|t| !t.is_bus())
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
        // Uneven times (logic signals, some CSV files): the FFT needs evenly
        // spaced samples, so the span is sampled at MAX_SAMPLES points.
        let uneven = matches!(ch.samples, Samples::Timed(_));
        let count = if uneven { fft::MAX_SAMPLES } else { count };
        if self.fft_cache.as_ref().map(|c| c.0) != Some(key) {
            let (values, dt) = if uneven {
                let (t0, t1) = (*span.start(), *span.end());
                let dt = (t1 - t0) / fft::MAX_SAMPLES as f64;
                let mut values = hold_resample(points, t0, dt, fft::MAX_SAMPLES);
                // A logic level is mostly DC: remove the mean, so that the
                // pulses, not the DC line, set the scale.
                if ch.is_digital() {
                    let mean = values.iter().sum::<f64>() / values.len() as f64;
                    values.iter_mut().for_each(|v| *v -= mean);
                }
                (values, dt)
            } else {
                let values: Vec<f64> = points[a..b.max(a)].iter().map(|p| p[1]).collect();
                (values, ch.sample_interval().unwrap_or(0.0))
            };
            self.fft_cache = Some((key, fft::spectrum(&values, dt, self.fft.window)));
        }
        let spectrum = self.fft_cache.as_ref()?.1.clone()?;
        Some((source, spectrum, count.min(fft::MAX_SAMPLES)))
    }

    /// The shortest time between two changes of trace `i` in the span shown.
    fn shortest_pulse(&self, i: usize) -> Option<f64> {
        let points = self.traces().get(i)?.points();
        let span = self.last_transform.as_ref()?.bounds().range_x();
        let a = points.partition_point(|p| p[0] < *span.start());
        let b = points.partition_point(|p| p[0] <= *span.end());
        points[a..b.max(a)]
            .windows(2)
            .map(|w| w[1][0] - w[0][0])
            .filter(|d| *d > 0.0)
            .min_by(f64::total_cmp)
    }

    /// Time step of the even sampling of the span shown, for the FFT of a
    /// trace with uneven times.
    fn fft_span_step(&self) -> Option<f64> {
        let span = self.last_transform.as_ref()?.bounds().range_x();
        Some((span.end() - span.start()) / fft::MAX_SAMPLES as f64)
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
        let peak = spectrum.peak();
        // A logic signal has no volts: levels in dB relative to the peak,
        // and only frequencies in the readouts.
        let logic = self.traces().get(source).is_some_and(Trace::is_digital);
        // A logic level that does not change in the span: no spectrum.
        if logic && peak.is_none_or(|(_, v)| v < 1e-9) {
            ui.label(
                RichText::new("FFT: the signal does not change in the time span shown")
                    .color(palette::hint()),
            );
            return;
        }
        let reference = peak.map_or(1.0, |(_, v)| v.max(f64::MIN_POSITIVE));
        let db = self.fft.db || logic;
        let level = move |v: f64| {
            if logic {
                fft::dbv(v / reference)
            } else if db {
                fft::dbv(v)
            } else {
                v
            }
        };
        let db_unit = if logic { "dB" } else { "dBV" };
        let fmt_level = move |v: f64| {
            if logic {
                String::new()
            } else if db {
                format!(" {:.1} dBV", fft::dbv(v))
            } else {
                format!(" {}", fmt_sig(v, "V", READOUT_DIGITS, 0.0))
            }
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(format!(
                    "FFT {} · {}",
                    self.traces().get(source).map_or("", |t| t.label.as_str()),
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
                        "{}{}",
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
            // Logic: the even sampling must be finer than the pulses.
            if logic && let Some(gap) = self.shortest_pulse(source) {
                let dt = self.fft_span_step().unwrap_or(0.0);
                if dt * 2.0 > gap {
                    ui.label(
                        RichText::new(format!(
                            "⚠ zoom in: the span shown is sampled every {}, the shortest pulse is {}",
                            fmt_sig(dt, "s", 3, 0.0),
                            fmt_sig(gap, "s", 3, 0.0)
                        ))
                        .color(palette::warn_text()),
                    );
                }
            }
            readout(ui, "N", Some(count.to_string()), palette::fg_dim()).on_hover_text(if logic {
                "Samples used: the time span shown, sampled evenly"
            } else {
                "Samples used (the time span shown)"
            });
            if ui
                .small_button("⛶ Fit")
                .on_hover_text(
                    "Show the spectrum up to 20 × the peak frequency, with all levels \
                     (the start view). Double-click on the FFT plot does the same.",
                )
                .clicked()
            {
                self.fft_fit = true;
            }
        });
        let points: Vec<[f64; 2]> = spectrum
            .freqs
            .iter()
            .zip(&spectrum.vrms)
            .map(|(&f, &v)| [f, level(v)])
            .collect();
        // Bars: every bin (drawn after the plot, only the part in view).
        let bars = self.fft.bars.then(|| points.clone());
        let bars_mode = bars.is_some();
        let points = decimate(&points, FFT_BUCKETS);
        // Start view: up to the 20th harmonic of the peak (the whole span up
        // to half the sample rate is often far wider than the signal).
        let nyquist = spectrum.freqs.last().copied().unwrap_or(1.0);
        let f_max = peak.map_or(nyquist, |(pf, _)| {
            (pf * 20.0).clamp(nyquist / 1000.0, nyquist)
        });
        // A new view when the highest frequency changes (an uneven-time
        // trace is sampled more finely when the time view is zoomed in).
        let span_key = nyquist.log2().round() as i64;
        let mut plot = Plot::new(("fft", self.load_count, source, db, span_key))
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
                    format!("{:.1} {db_unit}", p.y)
                } else {
                    si(p.y, "V")
                };
                Some(format!("f = {}\n{y}", si(p.x, "Hz")))
            });
        if std::mem::take(&mut self.fft_fit) {
            plot = plot.reset();
        }
        plot = if db {
            plot.y_axis_label(if logic { "dB (rel. to peak)" } else { "dBV" })
                .y_axis_formatter(|mark, _| format!("{:.0}", mark.value))
        } else {
            plot.y_axis_label("V RMS")
                .y_axis_formatter(|mark, _| si(mark.value, "V"))
        };
        let response = plot.show(ui, |plot| {
            // Mouse wheel zooms, like the time plot.
            let (delta, modifiers) = plot.ctx().input(|i| (wheel_delta(i), i.modifiers));
            let amount = zoom_amount(delta, modifiers);
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
            if bars_mode {
                // The line is kept for the hover label and the bounds, but
                // not drawn: alpha 1 (fully transparent means "automatic
                // colour" to egui_plot) and no width.
                plot.line(
                    Line::new("FFT", points)
                        .color(Color32::from_rgba_premultiplied(0, 0, 0, 1))
                        .width(0.0),
                );
            } else {
                plot.line(Line::new("FFT", points).color(color).width(1.2));
            }
            if let Some((pf, pv)) = peak {
                plot.points(
                    egui_plot::Points::new("", vec![[pf, level(pv)]])
                        .radius(4.0)
                        .color(palette::fg()),
                );
            }
        });
        if let Some(all) = bars {
            // dB: from the bottom of the plot; V RMS: from 0 V.
            draw_fft_bars(ui, &response.transform, &all, !db, color);
        }
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
        let screen = self.screen_window();
        let labels: Vec<String> = self.traces().iter().map(|t| t.label.clone()).collect();
        let digital: Vec<bool> = self.traces().iter().map(Trace::is_digital).collect();
        let buses: Vec<bool> = self.traces().iter().map(Trace::is_bus).collect();
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
        // Digital lanes: the y axis has no value scale there.
        let lane_top = self.lane_zone_top();
        let in_lanes = move |y: f64| lane_top.is_some_and(|z| y < z);
        let bus_hover = self.bus_hover(ui.ctx(), &maps);
        let logic_only = lane_top == Some(f64::INFINITY);
        let mut plot = Plot::new(("scope", self.load_count, self.y_axis));
        // Like a scope screen: 8 divisions. Not with logic signals only: the
        // lanes fill the plot (they would sit at the bottom of the 8 divisions).
        if divisions && !logic_only {
            plot = plot.default_y_bounds(-SCREEN_DIVS_Y / 2.0, SCREEN_DIVS_Y / 2.0);
        }
        plot = if divisions {
            // A line per division, and thinner lines at 1/5 division when
            // zoomed in.
            plot.y_axis_label(if logic_only { "Logic" } else { "Divisions" })
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
                .y_axis_formatter(move |mark, _| {
                    let v = mark.value;
                    if logic_only {
                        String::new()
                    } else if (v - v.round()).abs() < 1e-6 {
                        format!("{}", v.round() as i64)
                    } else {
                        String::new()
                    }
                })
        } else {
            plot.y_axis_label(if logic_only { "Logic" } else { "Voltage" })
                .y_axis_formatter(move |mark, _| {
                    if in_lanes(mark.value) {
                        String::new()
                    } else {
                        si(mark.value, "V")
                    }
                })
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
                            if let Some(text) = &bus_hover {
                                format!("{}\n{text}", t(position))
                            } else if in_lanes(position.y) {
                                t(position)
                            } else if divisions {
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
                // Over an axis, any wheel direction pans that axis.
                let axis_amount = delta.x + delta.y;
                let amount = zoom_amount(delta, modifiers);
                // A horizontal swipe (trackpad) inside the frame moves the time.
                let swipe = if modifiers.shift || modifiers.alt {
                    0.0
                } else {
                    delta.x
                };
                let frame = *plot.transform().frame();
                let zone = pointer.and_then(|p| axis_zone(p, frame, outer));
                // An axis label area pans that axis; inside the plot frame the wheel zooms.
                // (The plot response also covers the axis areas, so test the zones first.)
                // Not through a window or menu on top of the plot (Help).
                if (axis_amount != 0.0 || swipe != 0.0) && pointer_on_background(plot.ctx()) {
                    let amount = if zone.is_some() { axis_amount } else { amount };
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
                        if swipe != 0.0 {
                            let dx = plot.transform().dvalue_dpos()[0];
                            plot.translate_bounds(egui::vec2((-f64::from(swipe) * dx) as f32, 0.0));
                        }
                        let f = (amount / WHEEL_ZOOM_POINTS).exp();
                        // Logic signals only: the lanes keep their height.
                        let factor = if modifiers.shift || logic_only {
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
                    if fit == Fit::Screen
                        && let Some(s) = screen.clone()
                    {
                        plot.set_plot_bounds_x(s);
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
                    // A bus is drawn after the plot, with its values; here
                    // an invisible line keeps its lane in the automatic bounds.
                    if buses[i] {
                        plot.line(
                            // (Fully transparent means "automatic colour"
                            // to egui_plot: use alpha 1 and no width.)
                            Line::new("", points)
                                .color(Color32::from_rgba_premultiplied(0, 0, 0, 1))
                                .width(0.0)
                                .allow_band(false)
                                .allow_hover(false),
                        );
                    } else if !points.is_empty() {
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
        draw_cursor_tags(ui, &response.transform, &self.cursors);
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
        draw_bus_lanes(
            ui,
            &response.transform,
            self.traces(),
            &maps,
            self.bus_radix,
        );
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
/// The wheel amount that zooms: the vertical wheel only, so a horizontal
/// trackpad swipe pans instead. With Shift, browsers and some systems send
/// the wheel as horizontal, so both directions count.
/// `n` values at times `t0 + i·dt`: each the value of the last point at or
/// before that time (a logic signal holds its level until the next change).
pub(super) fn hold_resample(points: &[[f64; 2]], t0: f64, dt: f64, n: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(n);
    let mut k = 0;
    for i in 0..n {
        let t = t0 + i as f64 * dt;
        while k + 1 < points.len() && points[k + 1][0] <= t {
            k += 1;
        }
        out.push(points.get(k).map_or(0.0, |p| p[1]));
    }
    out
}

pub(super) fn zoom_amount(delta: egui::Vec2, modifiers: egui::Modifiers) -> f32 {
    if modifiers.shift {
        delta.x + delta.y
    } else {
        delta.y
    }
}

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
/// The name of each cursor at the edge of the plot: T1/T2 at the bottom of
/// their lines, V1/V2 at the right. A cursor outside the view shows an arrow
/// at the edge it is beyond ("◀T1", "V2▲"), so it is not lost.
pub(super) fn draw_cursor_tags(ui: &egui::Ui, transform: &PlotTransform, cursors: &Cursors) {
    let frame = *transform.frame();
    let painter = ui.painter_at(frame.expand(1.0));
    let font = egui::FontId::monospace(10.0);
    let tag = |pos: egui::Pos2, align: egui::Align2, text: String, color: egui::Color32| {
        let galley = painter.layout_no_wrap(text, font.clone(), color);
        let rect = align.anchor_size(pos, galley.size()).expand(1.5);
        painter.rect_filled(rect, 2.0, palette::screen_bg().gamma_multiply(0.85));
        painter.galley(rect.min + egui::vec2(1.5, 1.5), galley, color);
    };
    if cursors.mode.shows_time() {
        let color = palette::time_cursor();
        for (i, &t) in cursors.t.iter().enumerate() {
            let x = transform.position_from_point_x(t);
            let y = frame.bottom() - 2.0 - 13.0 * i as f32;
            let name = format!("T{}", i + 1);
            if x < frame.left() {
                tag(
                    egui::pos2(frame.left() + 2.0, y),
                    egui::Align2::LEFT_BOTTOM,
                    format!("◀{name}"),
                    color,
                );
            } else if x > frame.right() {
                tag(
                    egui::pos2(frame.right() - 2.0, y),
                    egui::Align2::RIGHT_BOTTOM,
                    format!("{name}▶"),
                    color,
                );
            } else {
                tag(
                    egui::pos2(x + 2.0, frame.bottom() - 2.0),
                    egui::Align2::LEFT_BOTTOM,
                    name,
                    color,
                );
            }
        }
    }
    if cursors.mode.shows_volt() {
        let color = palette::volt_cursor();
        for (i, &v) in cursors.v.iter().enumerate() {
            let y = transform.position_from_point_y(v);
            let x = frame.right() - 2.0 - 30.0 * i as f32;
            let name = format!("V{}", i + 1);
            if y < frame.top() {
                tag(
                    egui::pos2(x, frame.top() + 16.0),
                    egui::Align2::RIGHT_TOP,
                    format!("{name}▲"),
                    color,
                );
            } else if y > frame.bottom() {
                tag(
                    egui::pos2(x, frame.bottom() - 16.0),
                    egui::Align2::RIGHT_BOTTOM,
                    format!("{name}▼"),
                    color,
                );
            } else {
                tag(
                    egui::pos2(frame.right() - 2.0, y - 2.0),
                    egui::Align2::RIGHT_BOTTOM,
                    name,
                    color,
                );
            }
        }
    }
}

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

/// A part of a bus record with one value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BusSegment {
    pub start: f64,
    pub end: f64,
    /// The value; `None` for a part with changes too close to draw one by one.
    pub value: Option<f64>,
}

/// The segments of bus `points` (steps: two points at each change) that
/// cross `view`. Changes closer than `min_dt` make one `None` segment.
pub(super) fn bus_segments(
    points: &[[f64; 2]],
    view: &RangeInclusive<f64>,
    min_dt: f64,
) -> Vec<BusSegment> {
    let n = points.len();
    // The last point at or before `t`: it holds the value from `t` on.
    let last_at = |t: f64| points.partition_point(|p| p[0] <= t).saturating_sub(1);
    let mut out: Vec<BusSegment> = Vec::new();
    let mut i = last_at(*view.start());
    while i + 1 < n && points[i][0] <= *view.end() {
        let start = points[i][0];
        let j = i + points[i..].partition_point(|p| p[0] <= start);
        let Some(next) = points.get(j) else { break };
        let end = next[0];
        if end - start >= min_dt {
            out.push(BusSegment {
                start,
                end,
                value: Some(points[i][1]),
            });
            i = last_at(end);
            continue;
        }
        // Changes too close: one busy segment up to the change after
        // `start + min_dt`.
        let k = last_at(start + min_dt);
        let end = points[k][0];
        match out.last_mut() {
            Some(s) if s.value.is_none() && s.end == start => s.end = end,
            _ => out.push(BusSegment {
                start,
                end,
                value: None,
            }),
        }
        i = k;
    }
    out
}

/// The buses in their lanes: a hexagon per value, with the value written
/// in it when it fits; a filled band where the changes are too close.
pub(super) fn draw_bus_lanes(
    ui: &egui::Ui,
    transform: &PlotTransform,
    traces: &[Trace],
    maps: &[Option<YMap>],
    radix: Radix,
) {
    let frame = *transform.frame();
    let painter = ui.painter_at(frame);
    let view = transform.bounds().range_x();
    // Seconds per screen point.
    let dt_pt = transform.dvalue_dpos()[0].abs();
    let font = egui::FontId::monospace(11.0);
    for (i, (t, map)) in traces.iter().zip(maps).enumerate() {
        let (Some(bits), Some(map)) = (t.bus, map) else {
            continue;
        };
        let color = palette::channel(i);
        let stroke = egui::Stroke::new(1.5, color);
        let top = transform.position_from_point_y(map.to_plot(1.0));
        let bottom = transform.position_from_point_y(map.to_plot(0.0));
        let mid = (top + bottom) / 2.0;
        let slant = ((bottom - top) / 4.0).clamp(1.0, 4.0);
        // Out of the frame, the ends are not drawn (the painter clips).
        let x = |t: f64| {
            transform
                .position_from_point_x(t)
                .clamp(frame.left() - 20.0, frame.right() + 20.0)
        };
        for s in bus_segments(t.points(), &view, 3.0 * dt_pt) {
            let (x0, x1) = (x(s.start), x(s.end));
            let rect = egui::Rect::from_x_y_ranges(x0..=x1, top..=bottom);
            let Some(v) = s.value else {
                painter.rect_filled(rect, 0.0, color.gamma_multiply(0.45));
                continue;
            };
            let d = slant.min((x1 - x0) / 2.0);
            let shape = vec![
                egui::pos2(x0, mid),
                egui::pos2(x0 + d, top),
                egui::pos2(x1 - d, top),
                egui::pos2(x1, mid),
                egui::pos2(x1 - d, bottom),
                egui::pos2(x0 + d, bottom),
            ];
            // An unknown value (x or z bits): filled in the warning colour.
            let fill = if v.is_nan() {
                palette::warn().gamma_multiply(0.3)
            } else {
                Color32::TRANSPARENT
            };
            painter.add(egui::Shape::convex_polygon(shape, fill, stroke));
            // The value, in the middle of the visible part.
            let (a, b) = (
                (x0 + d).max(frame.left()) + 2.0,
                (x1 - d).min(frame.right()) - 2.0,
            );
            let galley = painter.layout_no_wrap(bus_text(v, bits, radix), font.clone(), color);
            if galley.size().x <= b - a && galley.size().y <= bottom - top + 2.0 {
                let pos = egui::pos2((a + b) / 2.0, mid) - galley.size() / 2.0;
                painter.galley(pos, galley, color);
            }
        }
    }
}

/// The FFT as bars: one bar per frequency bin of `points` (`[f, level]`),
/// from the bottom of the plot (or from level 0 when `from_zero`) up to its
/// level. When the bins are closer than 4 points on the screen, each screen
/// column gets one bar, at the highest level of its bins.
pub(super) fn draw_fft_bars(
    ui: &egui::Ui,
    transform: &PlotTransform,
    points: &[[f64; 2]],
    from_zero: bool,
    color: Color32,
) {
    let frame = *transform.frame();
    let painter = ui.painter_at(frame);
    let view = transform.bounds().range_x();
    let a = points
        .partition_point(|p| p[0] < *view.start())
        .saturating_sub(1);
    let b = (points.partition_point(|p| p[0] <= *view.end()) + 1).min(points.len());
    let shown = &points[a..b.max(a)];
    let base = if from_zero {
        transform
            .position_from_point_y(0.0)
            .clamp(frame.top(), frame.bottom())
    } else {
        frame.bottom()
    };
    let x = |f: f64| transform.position_from_point_x(f);
    // Top of a bar on the screen; `None` for a level not drawn (−∞ dB).
    let top = |v: f64| {
        v.is_finite()
            .then(|| transform.position_from_point_y(v).max(frame.top()))
    };
    let fill = color.gamma_multiply(0.85);
    let step = shown
        .windows(2)
        .map(|w| (x(w[1][0]) - x(w[0][0])).abs())
        .next()
        .unwrap_or(f32::INFINITY);
    if step >= 4.0 {
        // Bars with a gap of at least 1 point between them.
        let half = (step * 0.7).min(step - 1.0) / 2.0;
        for p in shown {
            let (Some(y), cx) = (top(p[1]), x(p[0])) else {
                continue;
            };
            let rect = egui::Rect::from_min_max(
                egui::pos2(cx - half, y.min(base)),
                egui::pos2(cx + half, y.max(base)),
            );
            painter.rect_filled(rect, 0.0, fill);
        }
        return;
    }
    // Bins closer than the bars: one bar per screen column.
    let stroke = egui::Stroke::new(1.0, fill);
    let mut column: Option<(i32, f32)> = None;
    let draw = |c: i32, y: f32| {
        let cx = c as f32 + 0.5;
        painter.line_segment([egui::pos2(cx, y), egui::pos2(cx, base)], stroke);
    };
    for p in shown {
        let Some(y) = top(p[1]) else { continue };
        let c = x(p[0]).floor() as i32;
        column = match column {
            Some((col, best)) if col == c => Some((col, best.min(y))),
            Some((col, best)) => {
                draw(col, best);
                Some((c, y))
            }
            None => Some((c, y)),
        };
    }
    if let Some((col, best)) = column {
        draw(col, best);
    }
}
