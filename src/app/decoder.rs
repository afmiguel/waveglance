//! The protocol decoder in the app: the words drawn just above the decoded
//! signal (like a logic analyzer), its menu and the row of decoded words.

use super::*;
use crate::decode::{self, Decoded, Frame, FrameKind, Protocol, uart};
use crate::model::Trace;

/// Most words written in a decoder row (the plot shows all of them).
const ROW_MAX_WORDS: usize = 400;
/// Decode results kept (one per decoded line and settings).
const CACHE_LEN: usize = 16;

/// What a decode result depends on: the file, the line, the settings and
/// the clock and chip select lines.
pub(super) type DecodeKey = (u64, String, decode::Settings, String, String);

/// The decoder of one line (saved between sessions, found by the name of
/// the line, so that it works again on the next file with that line).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct LineDecoder {
    pub label: String,
    pub on: bool,
    pub settings: decode::Settings,
    /// The clock line of a protocol that needs one (I2C: SCL, SPI:
    /// SCLK), by name.
    pub clock: String,
    /// The chip select line (SPI CS), by name; empty for none.
    pub select: String,
}

impl LineDecoder {
    /// A new decoder for line `label`, from the names of the lines: I2C
    /// for SDA (clock: SCL), SPI for MOSI, MISO, SDO or SDI (clock: SCLK,
    /// SCK or CLK; select: CS, SS or NSS), else UART.
    fn new(label: &str, labels: &[String]) -> Self {
        let name = |l: &str| l.to_lowercase();
        let find = |parts: &[&str]| {
            labels
                .iter()
                .find(|l| parts.iter().any(|p| name(l).contains(p)))
                .cloned()
                .unwrap_or_default()
        };
        let me = name(label);
        let (protocol, clock, select) = if me.contains("sda") {
            (Protocol::I2c, find(&["scl"]), String::new())
        } else if ["mosi", "miso", "sdo", "sdi", "copi", "cipo"]
            .iter()
            .any(|p| me.contains(p))
        {
            (
                Protocol::Spi,
                find(&["sclk", "sck", "clk"]),
                find(&["cs", "ss"]),
            )
        } else {
            (Protocol::Uart, String::new(), String::new())
        };
        Self {
            label: label.to_owned(),
            on: false,
            settings: decode::Settings {
                protocol,
                ..Default::default()
            },
            clock,
            select,
        }
    }
}

impl ScopeApp {
    /// The decoder of trace `i`, if it is on.
    fn line_decoder(&self, i: usize) -> Option<&LineDecoder> {
        let t = self.traces().get(i).filter(|t| !t.is_bus())?;
        self.decoders
            .iter()
            .find(|d| d.on && d.label == t.label && d.settings.protocol != Protocol::Off)
    }

    /// The protocol of the decoder of trace `i` (Off if none).
    pub(super) fn line_protocol(&self, i: usize) -> Protocol {
        self.line_decoder(i)
            .map_or(Protocol::Off, |d| d.settings.protocol)
    }

    /// The traces that are decoded, in trace order.
    pub(super) fn decoded_lines(&self) -> Vec<usize> {
        (0..self.traces().len())
            .filter(|&i| self.line_decoder(i).is_some())
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
        let line = self.line_decoder(i)?;
        let settings = line.settings;
        let trace = &self.traces()[i];
        let clock = settings
            .protocol
            .needs_clock()
            .then(|| {
                self.traces()
                    .iter()
                    .find(|t| t.label == line.clock && !t.is_bus())
            })
            .flatten();
        let select = (settings.protocol.uses_select() && !line.select.is_empty())
            .then(|| {
                self.traces()
                    .iter()
                    .find(|t| t.label == line.select && !t.is_bus())
            })
            .flatten();
        let key = (
            self.load_count,
            trace.label.clone(),
            settings,
            clock.map_or(String::new(), |c| c.label.clone()),
            select.map_or(String::new(), |c| c.label.clone()),
        );
        if let Some((_, d)) = self.decode_cache.borrow().iter().find(|(k, _)| *k == key) {
            return d.clone();
        }
        let d = decode::decode(trace, clock, select, &settings).map(Rc::new);
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
            Some(format!(
                "{} #{} {}",
                self.traces()[line].label,
                i + 1,
                describe(f)
            ))
        })
    }

    /// Contents of the Decode menu: a check box per line, and the settings
    /// of each decoded line in its own submenu.
    pub(super) fn decode_menu(&mut self, ui: &mut egui::Ui) {
        ui.label(tr("Decode the lines checked:"));
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
        // Lines that can be a clock: (index, name).
        let lines: Vec<(usize, String)> = self
            .traces()
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.is_bus())
            .map(|(i, t)| (i, t.label.clone()))
            .collect();
        let labels: Vec<String> = lines.iter().map(|(_, l)| l.clone()).collect();
        let mut decoders = self.decoders.clone();
        egui::Grid::new("decode_lines")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for (i, label, bus, threshold) in rows {
                    let k = match decoders.iter().position(|d| d.label == label) {
                        Some(k) => k,
                        None => {
                            decoders.push(LineDecoder::new(&label, &labels));
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
                    .on_disabled_hover_text(tr("A bus has values, not one line"));
                    if d.on && !bus {
                        ui.menu_button(format!("{} ▸", summary(d)), |ui| {
                            ui.label(
                                RichText::new(crate::trf!("Decoder of {label}", label = label))
                                    .strong(),
                            );
                            ui.horizontal(|ui| {
                                ui.label(tr("Protocol:"));
                                for p in Protocol::ALL {
                                    ui.radio_value(&mut d.settings.protocol, p, p.label());
                                }
                            });
                            ui.separator();
                            match d.settings.protocol {
                                Protocol::I2c => {
                                    ui.label(crate::trf!(
                                        "{label} is SDA (data). Clock line (SCL):",
                                        label = label
                                    ));
                                    ui.horizontal_wrapped(|ui| {
                                        for (j, name) in lines.iter().filter(|(j, _)| *j != i) {
                                            ui.radio_value(
                                                &mut d.clock,
                                                name.clone(),
                                                RichText::new(name).color(palette::channel(*j)),
                                            );
                                        }
                                    });
                                }
                                Protocol::Spi => {
                                    spi_settings(ui, &label, i, &lines, d);
                                }
                                _ => uart_settings(ui, &label, &mut d.settings.uart),
                            }
                            if !digital[i] {
                                ui.separator();
                                threshold_setting(ui, &mut d.settings.threshold, threshold);
                            }
                        })
                        .response
                        .on_hover_text(tr("The protocol and settings of this line"));
                    }
                    ui.end_row();
                }
            });
        // Lines that are not in this file keep their settings for later.
        self.decoders = decoders;
        ui.separator();
        ui.label(tr("Show:"));
        ui.horizontal(|ui| {
            for r in Radix::ALL {
                ui.radio_value(&mut self.decode_radix, r, r.label());
            }
        });
        ui.separator();
        ui.label(
            RichText::new(tr(
                "Each line has its own settings (▸). The words are drawn just above \
                 the signal and listed under the plot; click a word in the list to show it.",
            ))
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
            tag(
                ui,
                &format!("{} {label}", self.line_protocol(line).label()),
                color,
            );
            ui.label(RichText::new(&d.title).monospace().color(palette::fg_dim()));
            let words = d
                .frames
                .iter()
                .filter(|f| matches!(f.kind, FrameKind::Data | FrameKind::Address { .. }))
                .count();
            let mut count = crate::trf!("{words} words", words = words);
            if errors > 0 {
                count.push_str(&crate::trf!(", {errors} with errors", errors = errors));
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
                    .small_button(tr("⎘ Copy"))
                    .on_hover_text(crate::trf!(
                        "Copy all the words of {label}, as shown",
                        label = label
                    ))
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
            // ASCII text (UART): the characters next to each other.
            if radix == Radix::Ascii && is_text(d) {
                ui.spacing_mut().item_spacing.x = 0.0;
            }
            for (i, f) in d.frames.iter().take(ROW_MAX_WORDS).enumerate() {
                let c = if f.error.is_some() {
                    palette::warn_text()
                } else {
                    color
                };
                let tip = crate::trf!(
                    "{label} #{a0} at {a1}\n{a2}\nClick to show it in the plot",
                    label = label,
                    a0 = i + 1,
                    a1 = si(f.start, "s"),
                    a2 = describe(f)
                );
                let r = ui
                    .add(
                        egui::Label::new(RichText::new(frame_text(f, radix)).monospace().color(c))
                            .sense(egui::Sense::click()),
                    )
                    .on_hover_text(tip);
                if r.clicked() {
                    self.pending_view = Some(ViewCommand::CenterX((f.start + f.end) / 2.0));
                }
            }
            if d.frames.len() > ROW_MAX_WORDS {
                ui.label(
                    RichText::new(crate::trf!(
                        " … {a0} more",
                        a0 = d.frames.len() - ROW_MAX_WORDS
                    ))
                    .color(palette::hint()),
                );
            }
        });
    }
}

/// The settings of a line in short: "UART Auto 8N1", "UART 19200 7E1 inv",
/// "I2C SCL: scl".
fn summary(d: &LineDecoder) -> String {
    if d.settings.protocol == Protocol::Spi {
        return if d.clock.is_empty() {
            tr("SPI, choose SCLK").to_owned()
        } else {
            crate::trf!(
                "SPI mode {a0}, SCLK: {a1}",
                a0 = d.settings.spi.mode,
                a1 = d.clock
            )
        };
    }
    if d.settings.protocol == Protocol::I2c {
        return if d.clock.is_empty() {
            tr("I2C, choose SCL").to_owned()
        } else {
            format!("I2C SCL: {}", d.clock)
        };
    }
    let u = &d.settings.uart;
    let baud = if u.baud == 0 {
        tr("Auto").to_owned()
    } else {
        u.baud.to_string()
    };
    let parity = match u.parity {
        uart::Parity::None => 'N',
        uart::Parity::Even => 'E',
        uart::Parity::Odd => 'O',
    };
    let mut text = format!("UART {baud} {}{parity}{}", u.data_bits, u.stop_bits);
    if u.idle_low {
        text.push_str(" inv");
    }
    text
}

/// The threshold of an analog line: Auto, or a value in volts. `used`: the
/// threshold of the last decoding.
fn threshold_setting(ui: &mut egui::Ui, threshold: &mut Option<f64>, used: Option<f64>) {
    ui.horizontal(|ui| {
        ui.label(tr("Threshold:"));
        let mut auto = threshold.is_none();
        if ui
            .checkbox(&mut auto, tr("Auto"))
            .on_hover_text(tr("The middle between the lowest and the highest sample"))
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

/// The SPI settings of data line `label` (trace `i`): clock, chip select,
/// mode, bit order and word size. `lines`: the lines that can be chosen.
fn spi_settings(
    ui: &mut egui::Ui,
    label: &str,
    i: usize,
    lines: &[(usize, String)],
    d: &mut LineDecoder,
) {
    ui.label(crate::trf!(
        "{label} is a data line (MOSI or MISO).",
        label = label
    ));
    let others = || lines.iter().filter(move |(j, _)| *j != i);
    egui::Grid::new(("spi_settings", label))
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label(tr("Clock (SCLK):"));
            ui.horizontal_wrapped(|ui| {
                for (j, name) in others() {
                    ui.radio_value(
                        &mut d.clock,
                        name.clone(),
                        RichText::new(name).color(palette::channel(*j)),
                    );
                }
            });
            ui.end_row();
            ui.label(tr("Chip select (CS):"));
            ui.horizontal_wrapped(|ui| {
                ui.radio_value(&mut d.select, String::new(), tr("None"))
                    .on_hover_text(tr("Without CS, a pause of the clock starts a new word"));
                for (j, name) in others().filter(|(_, n)| *n != d.clock) {
                    ui.radio_value(
                        &mut d.select,
                        name.clone(),
                        RichText::new(name).color(palette::channel(*j)),
                    );
                }
            });
            ui.end_row();
            let s = &mut d.settings.spi;
            if !d.select.is_empty() {
                ui.label(tr("CS active:"));
                ui.horizontal(|ui| {
                    ui.radio_value(&mut s.cs_active_low, true, tr("Low"));
                    ui.radio_value(&mut s.cs_active_low, false, tr("High"));
                });
                ui.end_row();
            }
            ui.label(tr("Mode:"));
            ui.horizontal(|ui| {
                for (m, tip) in [
                    (
                        0,
                        tr("CPOL 0, CPHA 0: clock idle low, read on the rising edge"),
                    ),
                    (
                        1,
                        tr("CPOL 0, CPHA 1: clock idle low, read on the falling edge"),
                    ),
                    (
                        2,
                        tr("CPOL 1, CPHA 0: clock idle high, read on the falling edge"),
                    ),
                    (
                        3,
                        tr("CPOL 1, CPHA 1: clock idle high, read on the rising edge"),
                    ),
                ] {
                    ui.radio_value(&mut s.mode, m, m.to_string())
                        .on_hover_text(tip);
                }
            });
            ui.end_row();
            ui.label(tr("Bit order:"));
            ui.horizontal(|ui| {
                ui.radio_value(&mut s.msb_first, true, tr("MSB first"));
                ui.radio_value(&mut s.msb_first, false, tr("LSB first"));
            });
            ui.end_row();
            ui.label(tr("Word size:"));
            ui.add(
                egui::DragValue::new(&mut s.word_bits)
                    .range(4..=16)
                    .suffix(" bits"),
            );
            ui.end_row();
        });
}

/// The UART settings of line `label`.
fn uart_settings(ui: &mut egui::Ui, label: &str, s: &mut uart::Settings) {
    egui::Grid::new(("uart_settings", label))
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label(tr("Baud:"));
            let text = if s.baud == 0 {
                tr("Auto").to_owned()
            } else {
                s.baud.to_string()
            };
            // A submenu, not a ComboBox: a ComboBox opens a popup of its
            // own, and a click in it closes the Decode menu.
            ui.menu_button(format!("{text} ▸"), |ui| {
                ui.selectable_value(&mut s.baud, 0, tr("Auto"))
                    .on_hover_text(tr("Measured from the shortest pulse"));
                ui.separator();
                for b in uart::STANDARD_BAUDS {
                    ui.selectable_value(&mut s.baud, b, b.to_string());
                }
            });
            ui.end_row();
            ui.label(tr("Data bits:"));
            ui.horizontal(|ui| {
                for b in 5..=9 {
                    ui.radio_value(&mut s.data_bits, b, b.to_string());
                }
            });
            ui.end_row();
            ui.label(tr("Parity:"));
            ui.horizontal(|ui| {
                for p in uart::Parity::ALL {
                    ui.radio_value(&mut s.parity, p, p.label());
                }
            });
            ui.end_row();
            ui.label(tr("Stop bits:"));
            ui.horizontal(|ui| {
                ui.radio_value(&mut s.stop_bits, 1, "1");
                ui.radio_value(&mut s.stop_bits, 2, "2");
            });
            ui.end_row();
            ui.label(tr("Idle level:"));
            ui.horizontal(|ui| {
                ui.radio_value(&mut s.idle_low, false, tr("High"))
                    .on_hover_text(tr("TTL serial of a microcontroller"));
                ui.radio_value(&mut s.idle_low, true, tr("Low (inverted)"))
                    .on_hover_text(tr("RS-232 line levels (before the transceiver)"));
            });
            ui.end_row();
        });
}

/// All the words as text: separated by spaces, or one string for ASCII
/// text (UART: data words only).
fn words_text(d: &Decoded, radix: Radix) -> String {
    let words = d.frames.iter().map(|f| frame_text(f, radix));
    if radix == Radix::Ascii && is_text(d) {
        words.collect()
    } else {
        words.collect::<Vec<_>>().join(" ")
    }
}

/// The frames are plain words (UART), that ASCII can show as text.
fn is_text(d: &Decoded) -> bool {
    d.frames
        .iter()
        .all(|f| f.kind == FrameKind::Data && f.ack.is_none())
}

/// A frame as shown in the plot and in the row: the word in `radix`; for
/// I2C, "S", "Sr" and "P" for start, repeated start and stop; for SPI,
/// "[" and "]" for CS active and released; "W 48" or
/// "R 48" for an address (in hex when `radix` is ASCII), and "N" after a
/// byte that was not acknowledged.
pub(super) fn frame_text(f: &Frame, radix: Radix) -> String {
    let nack = if f.ack == Some(false) { " N" } else { "" };
    match f.kind {
        FrameKind::Start { repeated: false } => "S".to_owned(),
        FrameKind::Start { repeated: true } => tr("Sr").to_owned(),
        FrameKind::Stop => "P".to_owned(),
        FrameKind::Select { active: true } => "[".to_owned(),
        FrameKind::Select { active: false } => "]".to_owned(),
        FrameKind::Address { read } => {
            let radix = if radix == Radix::Ascii {
                Radix::Hex
            } else {
                radix
            };
            format!(
                "{} {}{nack}",
                if read { "R" } else { "W" },
                word_text(f.value, f.bits, radix)
            )
        }
        FrameKind::Data => format!("{}{nack}", word_text(f.value, f.bits, radix)),
    }
}

/// A frame in words, for the hover text: what it is, its value in hex,
/// decimal and ASCII, the acknowledge, and the error.
fn describe(f: &Frame) -> String {
    let value = || {
        crate::trf!(
            "{a0} (hex) = {a1} = '{a2}'",
            a0 = word_text(f.value, f.bits, Radix::Hex),
            a1 = f.value,
            a2 = decode::ascii(f.value)
        )
    };
    let mut text = match f.kind {
        FrameKind::Start { repeated: false } => tr("start").to_owned(),
        FrameKind::Start { repeated: true } => tr("repeated start").to_owned(),
        FrameKind::Stop => tr("stop").to_owned(),
        FrameKind::Select { active: true } => tr("CS active (start of a selection)").to_owned(),
        FrameKind::Select { active: false } => tr("CS released").to_owned(),
        FrameKind::Address { read } => crate::trf!(
            "address {a0} (hex), {a1}",
            a0 = word_text(f.value, 7, Radix::Hex),
            a1 = if read { tr("read") } else { tr("write") }
        ),
        FrameKind::Data => format!("= {}", value()),
    };
    match f.ack {
        Some(true) => text.push_str(", ACK"),
        Some(false) => text.push_str(", NACK"),
        None => {}
    }
    if let Some(e) = f.error {
        text.push_str(&format!("\n⚠ {}", e.label()));
    }
    text
}

/// Colours of the I2C start (and repeated start) and stop dots.
const START_COLOR: Color32 = Color32::from_rgb(70, 200, 110);
const STOP_COLOR: Color32 = Color32::from_rgb(235, 80, 80);

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
        // I2C start and stop: a dot where the condition is (green for a
        // start, red for a stop), like a logic analyzer.
        let dot = match f.kind {
            FrameKind::Start { .. } | FrameKind::Select { active: true } => Some((x0, START_COLOR)),
            FrameKind::Stop | FrameKind::Select { active: false } => Some((x1, STOP_COLOR)),
            _ => None,
        };
        if let Some((at, dot_color)) = dot {
            let r = ((bottom - top) / 4.0).clamp(2.5, 5.0);
            painter.circle_filled(egui::pos2(at, mid), r, dot_color);
            continue;
        }
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
        let galley = painter.layout_no_wrap(frame_text(f, radix), font.clone(), text_color);
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
