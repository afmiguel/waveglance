//! The protocol decoder in the app: the words drawn just above the decoded
//! signal (like a logic analyzer), its menu and the row of decoded words.

use super::*;
use crate::decode::{self, Decoded, Protocol, uart};
use crate::model::Trace;

/// Most words written in a decoder row (the plot shows all of them).
const ROW_MAX_WORDS: usize = 400;
/// Decode results kept (one per decoded line and settings).
const CACHE_LEN: usize = 16;

/// What a decode result depends on: the file, the line and the settings.
pub(super) type DecodeKey = (u64, String, decode::Settings);

/// The decoder of one line (saved between sessions, found by the name of
/// the line, so that it works again on the next file with that line).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct LineDecoder {
    pub label: String,
    pub on: bool,
    pub settings: decode::Settings,
}

impl LineDecoder {
    fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            on: false,
            settings: decode::Settings {
                protocol: Protocol::Uart,
                ..Default::default()
            },
        }
    }
}

impl ScopeApp {
    /// The settings of the decoder of trace `i`, if it is on.
    fn line_settings(&self, i: usize) -> Option<decode::Settings> {
        let t = self.traces().get(i).filter(|t| !t.is_bus())?;
        self.decoders
            .iter()
            .find(|d| d.on && d.label == t.label && d.settings.protocol != Protocol::Off)
            .map(|d| d.settings)
    }

    /// The traces that are decoded, in trace order.
    pub(super) fn decoded_lines(&self) -> Vec<usize> {
        (0..self.traces().len())
            .filter(|&i| self.line_settings(i).is_some())
            .collect()
    }

    /// Decoded digital traces: each has `DECODE_ROOM` lanes of room above
    /// it for its words.
    pub(super) fn decode_lanes(&self) -> usize {
        self.decoded_lines()
            .into_iter()
            .filter(|&i| self.traces()[i].is_digital())
            .count()
    }

    /// The decode result of trace `i` (kept until the file or the
    /// settings change).
    pub(super) fn decoded_at(&self, i: usize) -> Option<Rc<Decoded>> {
        let settings = self.line_settings(i)?;
        let trace = &self.traces()[i];
        let key = (self.load_count, trace.label.clone(), settings);
        if let Some((_, d)) = self.decode_cache.borrow().iter().find(|(k, _)| *k == key) {
            return d.clone();
        }
        let d = decode::decode(trace, &settings).map(Rc::new);
        let mut cache = self.decode_cache.borrow_mut();
        cache.retain(|(k, _)| k.0 == self.load_count);
        if cache.len() >= CACHE_LEN {
            cache.remove(0);
        }
        cache.push((key, d.clone()));
        d
    }

    /// The room for the decoded words, in plot units: (near, far), `near`
    /// on the side of the signal. The words are drawn from `near`, with a
    /// fixed height on the screen. Just above the signal of trace `i`. A
    /// digital line: in the room kept free above it. An
    /// analog channel: above its highest sample (in Div mode, below its
    /// lowest sample when there is no room above in the 8 divisions).
    /// `None` if the trace is hidden.
    pub(super) fn decode_band(&self, i: usize) -> Option<(f64, f64)> {
        let map = self.y_maps().get(i).copied().flatten()?;
        let trace = &self.traces()[i];
        if trace.is_digital() {
            // One lane of room: from just above the high level.
            let lanes = self.total_lanes().max(2.0);
            let pitch = self.lane_map(1.0, lanes).offset - self.lane_map(0.0, lanes).offset;
            let top = map.to_plot(1.0);
            // The gap between lanes (0.3) and the room kept above the line.
            return Some((top + 0.06 * pitch, top + (0.3 + DECODE_ROOM - 0.04) * pitch));
        }
        let d = self.decoded_at(i)?;
        if !d.high.is_finite() {
            return None;
        }
        let top = map.to_plot(d.high);
        Some(match self.y_axis {
            YAxis::Divisions => {
                let edge = SCREEN_DIVS_Y / 2.0;
                let above = (top + 0.15, top + 0.75);
                let bottom = map.to_plot(d.low);
                let below = (bottom - 0.15, bottom - 0.75);
                if above.1 > edge && below.1 >= -edge {
                    below
                } else {
                    above
                }
            }
            YAxis::Volts => {
                let (lo, hi) = self.analog_span.unwrap_or((0.0, 1.0));
                let span = if hi > lo { hi - lo } else { 1.0 };
                (top + 0.03 * span, top + 0.2 * span)
            }
        })
    }

    /// The decoded word under the pointer, for the hover label.
    pub(super) fn decode_hover(&self, ctx: &egui::Context) -> Option<String> {
        let transform = self.last_transform.as_ref()?;
        let pos = ctx.input(|i| i.pointer.latest_pos())?;
        let at = transform.value_from_position(pos);
        self.decoded_lines().into_iter().find_map(|line| {
            let (near, far) = band_pixels(transform, self.decode_band(line)?);
            if !(near.min(far)..=near.max(far)).contains(&pos.y) {
                return None;
            }
            let d = self.decoded_at(line)?;
            let i = d.frames.partition_point(|f| f.end < at.x);
            let f = d.frames.get(i).filter(|f| f.start <= at.x)?;
            let mut text = format!(
                "{} #{} = {} (hex) = {} = '{}'",
                self.traces()[line].label,
                i + 1,
                word_text(f.value, f.bits, Radix::Hex),
                f.value,
                decode::ascii(f.value)
            );
            if let Some(e) = f.error {
                text.push_str(&format!("\n⚠ {}", e.label()));
            }
            Some(text)
        })
    }

    /// Contents of the Decode menu: a check box per line, and the settings
    /// of each decoded line in its own submenu.
    pub(super) fn decode_menu(&mut self, ui: &mut egui::Ui) {
        ui.label("UART on the lines checked:");
        // What the rows need, read before the decoders are changed.
        let rows: Vec<(usize, String, bool, Option<f64>)> = self
            .traces()
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let threshold = self.decoded_at(i).map(|d| d.threshold);
                (i, t.label.clone(), t.is_bus(), threshold)
            })
            .collect();
        let digital: Vec<bool> = self.traces().iter().map(Trace::is_digital).collect();
        let mut decoders = self.decoders.clone();
        egui::Grid::new("decode_lines")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for (i, label, bus, threshold) in rows {
                    let k = match decoders.iter().position(|d| d.label == label) {
                        Some(k) => k,
                        None => {
                            decoders.push(LineDecoder::new(&label));
                            decoders.len() - 1
                        }
                    };
                    let d = &mut decoders[k];
                    ui.add_enabled(
                        !bus,
                        egui::Checkbox::new(
                            &mut d.on,
                            RichText::new(&label).color(palette::channel(i)),
                        ),
                    )
                    .on_disabled_hover_text("A bus has values, not one line");
                    if d.on && !bus {
                        ui.menu_button(format!("{} ▸", summary(&d.settings)), |ui| {
                            ui.label(RichText::new(format!("UART on {label}")).strong());
                            uart_settings(ui, &label, &mut d.settings.uart);
                            if !digital[i] {
                                ui.separator();
                                threshold_setting(ui, &mut d.settings.threshold, threshold);
                            }
                        })
                        .response
                        .on_hover_text("The UART settings of this line");
                    }
                    ui.end_row();
                }
            });
        // Lines that are not in this file keep their settings for later.
        self.decoders = decoders;
        ui.separator();
        ui.label("Show:");
        ui.horizontal(|ui| {
            for r in Radix::ALL {
                ui.radio_value(&mut self.decode_radix, r, r.label());
            }
        });
        ui.separator();
        ui.label(
            RichText::new(
                "Each line has its own settings (▸). The words are drawn just above \
                 the signal and listed under the plot; click a word in the list to show it.",
            )
            .small()
            .color(palette::hint()),
        );
    }

    /// The rows of decoded words under the plot: one block per line.
    pub(super) fn decode_readout(&mut self, ui: &mut egui::Ui) {
        for (n, line) in self.decoded_lines().into_iter().enumerate() {
            if let Some(d) = self.decoded_at(line) {
                self.decode_line_readout(ui, line, &d, n == 0);
            }
        }
    }

    /// The block of one decoded line: its name, settings, word count, and
    /// the words. `first`: also the Hex/Dec/Bin/ASCII choice.
    fn decode_line_readout(&mut self, ui: &mut egui::Ui, line: usize, d: &Decoded, first: bool) {
        let color = palette::channel(line);
        let label = self.traces()[line].label.clone();
        let radix = self.decode_radix;
        let errors = d.frames.iter().filter(|f| f.error.is_some()).count();
        ui.horizontal_wrapped(|ui| {
            tag(ui, &format!("UART {label}"), color);
            ui.label(RichText::new(&d.title).monospace().color(palette::fg_dim()));
            let mut count = format!("{} words", d.frames.len());
            if errors > 0 {
                count.push_str(&format!(", {errors} with errors"));
            }
            ui.label(RichText::new(count).monospace().color(if errors > 0 {
                palette::warn_text()
            } else {
                palette::fg_dim()
            }));
            if first {
                for r in Radix::ALL {
                    ui.radio_value(&mut self.decode_radix, r, r.label());
                }
            }
            if !d.frames.is_empty()
                && ui
                    .small_button("⎘ Copy")
                    .on_hover_text(format!("Copy all the words of {label}, as shown"))
                    .clicked()
            {
                ui.ctx().copy_text(words_text(d, radix));
            }
        });
        for w in &d.warnings {
            ui.label(RichText::new(format!("⚠ {w}")).color(palette::warn_text()));
        }
        if d.frames.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            // ASCII: the characters next to each other, like text.
            if radix == Radix::Ascii {
                ui.spacing_mut().item_spacing.x = 0.0;
            }
            for (i, f) in d.frames.iter().take(ROW_MAX_WORDS).enumerate() {
                let c = if f.error.is_some() {
                    palette::warn_text()
                } else {
                    color
                };
                let mut tip = format!(
                    "{label} #{} at {}\n{} (hex) = {} = '{}'",
                    i + 1,
                    si(f.start, "s"),
                    word_text(f.value, f.bits, Radix::Hex),
                    f.value,
                    decode::ascii(f.value)
                );
                if let Some(e) = f.error {
                    tip.push_str(&format!("\n⚠ {}", e.label()));
                }
                tip.push_str("\nClick to show it in the plot");
                let r = ui
                    .add(
                        egui::Label::new(
                            RichText::new(word_text(f.value, f.bits, radix))
                                .monospace()
                                .color(c),
                        )
                        .sense(egui::Sense::click()),
                    )
                    .on_hover_text(tip);
                if r.clicked() {
                    self.pending_view = Some(ViewCommand::CenterX((f.start + f.end) / 2.0));
                }
            }
            if d.frames.len() > ROW_MAX_WORDS {
                ui.label(
                    RichText::new(format!(" … {} more", d.frames.len() - ROW_MAX_WORDS))
                        .color(palette::hint()),
                );
            }
        });
    }
}

/// The settings of a line in short: "Auto 8N1", "19200 7E1 inv".
fn summary(s: &decode::Settings) -> String {
    let u = &s.uart;
    let baud = if u.baud == 0 {
        "Auto".to_owned()
    } else {
        u.baud.to_string()
    };
    let parity = match u.parity {
        uart::Parity::None => 'N',
        uart::Parity::Even => 'E',
        uart::Parity::Odd => 'O',
    };
    let mut text = format!("{baud} {}{parity}{}", u.data_bits, u.stop_bits);
    if u.idle_low {
        text.push_str(" inv");
    }
    text
}

/// The threshold of an analog line: Auto, or a value in volts. `used`: the
/// threshold of the last decoding.
fn threshold_setting(ui: &mut egui::Ui, threshold: &mut Option<f64>, used: Option<f64>) {
    ui.horizontal(|ui| {
        ui.label("Threshold:");
        let mut auto = threshold.is_none();
        if ui
            .checkbox(&mut auto, "Auto")
            .on_hover_text("The middle between the lowest and the highest sample")
            .changed()
        {
            *threshold = if auto {
                None
            } else {
                Some(used.unwrap_or(0.0))
            };
        }
        if let Some(v) = threshold {
            ui.add(
                egui::DragValue::new(v)
                    .speed(0.01)
                    .suffix(" V")
                    .max_decimals(3),
            );
        } else if let Some(v) = used {
            ui.label(
                RichText::new(si(v, "V"))
                    .monospace()
                    .color(palette::fg_dim()),
            );
        }
    });
}

/// The UART settings of line `label`.
fn uart_settings(ui: &mut egui::Ui, label: &str, s: &mut uart::Settings) {
    egui::Grid::new(("uart_settings", label))
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Baud:");
            let text = if s.baud == 0 {
                "Auto".to_owned()
            } else {
                s.baud.to_string()
            };
            // A submenu, not a ComboBox: a ComboBox opens a popup of its
            // own, and a click in it closes the Decode menu.
            ui.menu_button(format!("{text} ▸"), |ui| {
                ui.selectable_value(&mut s.baud, 0, "Auto")
                    .on_hover_text("Measured from the shortest pulse");
                ui.separator();
                for b in uart::STANDARD_BAUDS {
                    ui.selectable_value(&mut s.baud, b, b.to_string());
                }
            });
            ui.end_row();
            ui.label("Data bits:");
            ui.horizontal(|ui| {
                for b in 5..=9 {
                    ui.radio_value(&mut s.data_bits, b, b.to_string());
                }
            });
            ui.end_row();
            ui.label("Parity:");
            ui.horizontal(|ui| {
                for p in uart::Parity::ALL {
                    ui.radio_value(&mut s.parity, p, p.label());
                }
            });
            ui.end_row();
            ui.label("Stop bits:");
            ui.horizontal(|ui| {
                ui.radio_value(&mut s.stop_bits, 1, "1");
                ui.radio_value(&mut s.stop_bits, 2, "2");
            });
            ui.end_row();
            ui.label("Idle level:");
            ui.horizontal(|ui| {
                ui.radio_value(&mut s.idle_low, false, "High")
                    .on_hover_text("TTL serial of a microcontroller");
                ui.radio_value(&mut s.idle_low, true, "Low (inverted)")
                    .on_hover_text("RS-232 line levels (before the transceiver)");
            });
            ui.end_row();
        });
}

/// All the words as text: separated by spaces, or one string for ASCII.
fn words_text(d: &Decoded, radix: Radix) -> String {
    let words = d.frames.iter().map(|f| word_text(f.value, f.bits, radix));
    if radix == Radix::Ascii {
        words.collect()
    } else {
        words.collect::<Vec<_>>().join(" ")
    }
}

/// Height of the words on the screen, in points.
const WORD_HEIGHT: f32 = 22.0;

/// The screen y of the words (near, far): from the side of the signal,
/// `WORD_HEIGHT` points (less if the room is smaller).
fn band_pixels(transform: &PlotTransform, band: (f64, f64)) -> (f32, f32) {
    let near = transform.position_from_point_y(band.0);
    let far = transform.position_from_point_y(band.1);
    let h = (far - near).abs().min(WORD_HEIGHT);
    (near, near + h * (far - near).signum())
}

/// The decoded words in their band above the signal: a hexagon per word
/// with its text when it fits; a filled band where the words are too close;
/// a warning colour for a word with an error.
pub(super) fn draw_decode_lane(
    ui: &egui::Ui,
    transform: &PlotTransform,
    d: &Decoded,
    band: (f64, f64),
    radix: Radix,
    color: Color32,
    label: &str,
) {
    let frame = *transform.frame();
    let painter = ui.painter_at(frame);
    let view = transform.bounds().range_x();
    let (near, far) = band_pixels(transform, band);
    let (top, bottom) = (near.min(far), near.max(far));
    let mid = (top + bottom) / 2.0;
    let stroke = egui::Stroke::new(1.5, color);
    let slant = ((bottom - top) / 4.0).clamp(1.0, 4.0);
    let x = |t: f64| {
        transform
            .position_from_point_x(t)
            .clamp(frame.left() - 20.0, frame.right() + 20.0)
    };
    let font = egui::FontId::monospace(((bottom - top) * 0.62).clamp(9.0, 14.0));
    let first = d.frames.partition_point(|f| f.end < *view.start());
    let mut busy: Option<(f32, f32)> = None;
    let flush = |busy: &mut Option<(f32, f32)>| {
        if let Some((a, b)) = busy.take() {
            let rect = egui::Rect::from_x_y_ranges(a..=b.max(a + 1.0), top..=bottom);
            painter.rect_filled(rect, 0.0, color.gamma_multiply(0.45));
        }
    };
    for f in d.frames[first..]
        .iter()
        .take_while(|f| f.start <= *view.end())
    {
        let (x0, x1) = (x(f.start), x(f.end));
        // Too narrow to draw one by one: one band.
        if x1 - x0 < 3.0 {
            busy = Some(busy.map_or((x0, x1), |(a, _)| (a, x1)));
            continue;
        }
        flush(&mut busy);
        let d = slant.min((x1 - x0) / 2.0);
        let shape = vec![
            egui::pos2(x0, mid),
            egui::pos2(x0 + d, top),
            egui::pos2(x1 - d, top),
            egui::pos2(x1, mid),
            egui::pos2(x1 - d, bottom),
            egui::pos2(x0 + d, bottom),
        ];
        let (fill, text_color) = if f.error.is_some() {
            (palette::warn().gamma_multiply(0.3), palette::warn_text())
        } else {
            (palette::screen_bg(), color)
        };
        painter.add(egui::Shape::convex_polygon(shape, fill, stroke));
        let (a, b) = (
            (x0 + d).max(frame.left()) + 2.0,
            (x1 - d).min(frame.right()) - 2.0,
        );
        let galley =
            painter.layout_no_wrap(word_text(f.value, f.bits, radix), font.clone(), text_color);
        if galley.size().x <= b - a && galley.size().y <= bottom - top + 2.0 {
            let pos = egui::pos2((a + b) / 2.0, mid) - galley.size() / 2.0;
            painter.galley(pos, galley, text_color);
        }
    }
    flush(&mut busy);
    // The name on the far side of the words (away from the signal).
    let (y, align) = if far < near {
        (top - 2.0, egui::Align2::LEFT_BOTTOM)
    } else {
        (bottom + 2.0, egui::Align2::LEFT_TOP)
    };
    painter.text(
        egui::pos2(frame.left() + 4.0, y),
        align,
        label,
        egui::FontId::monospace(10.0),
        color,
    );
}
