//! Windows: welcome screen, Help, Print, statistics and the error banner.

use super::*;

impl ScopeApp {
    /// The splash screen: what the app does, its version, the built-in
    /// examples (a click opens one) and the controls. "Don't show this at
    /// startup" is saved; the Help button has the full instructions.
    pub(super) fn splash(&mut self, ctx: &egui::Context) {
        if !self.splash_open {
            return;
        }
        let mut open_file = false;
        let mut example: Option<&'static examples::Example> = None;
        let screen = ctx.content_rect();
        let max_height = screen.height() * 0.85;
        // As wide as the screen allows (a phone), at most 680 points.
        let width = (screen.width() - 32.0).clamp(150.0, 680.0);
        let modal = egui::Modal::new(egui::Id::unique("splash")).show(ctx, |ui| {
            ui.set_width(width);
            ui.heading(RichText::new("WaveGlance").strong());
            ui.label(
                RichText::new(format!("v{VERSION} · commit {GIT_HASH}"))
                    .small()
                    .color(palette::hint()),
            );
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height((max_height - 170.0).max(120.0))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.label(
                        "A quick look at oscilloscope and logic analyzer files: FNIRSI DPOX180H and \
                         1013D/1014D (.wav), Keysight/Agilent (.bin), Rigol (.bin, .wfm), VCD, and CSV \
                         files from any oscilloscope. Measurements, cursors, FFT, Math, search, and \
                         UART, I2C and SPI decoders. The files are read on this device: nothing is \
                         uploaded.",
                    );
                    ui.add_space(8.0);
                    ui.label(RichText::new("Try an example").strong().size(15.0));
                    ui.label(
                        RichText::new("Click one to open it. File ▾ → Open example has them too.")
                            .small()
                            .color(palette::hint()),
                    );
                    ui.add_space(4.0);
                    if let Some(e) = example_cards(ui) {
                        example = Some(e);
                    }
                    ui.add_space(6.0);
                    egui::CollapsingHeader::new(RichText::new("Controls").strong())
                        .default_open(false)
                        .show(ui, controls_grid);
                });
            ui.add_space(8.0);
            ui.checkbox(&mut self.hide_splash, "Don't show this at startup");
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
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
        if let Some(e) = example {
            self.splash_open = false;
            self.load_bytes(e.file.to_owned(), e.bytes);
        }
        if open_file {
            self.splash_open = false;
            self.open_file_dialog(ctx);
        }
    }

    /// The Help window: the main instructions, in short sections.
    pub(super) fn help(&mut self, ctx: &egui::Context) {
        let mut open = self.help_open;
        let mut welcome = false;
        egui::Window::new("Help")
            .open(&mut open)
            .default_width(480.0)
            .default_height(ctx.content_rect().height() * 0.7)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                if ui
                    .button("☰ Welcome screen and examples")
                    .on_hover_text("The welcome screen, with the built-in examples")
                    .clicked()
                {
                    welcome = true;
                }
                egui::ScrollArea::vertical().show(ui, help_text);
            });
        if welcome {
            open = false;
            self.splash_open = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }
        self.help_open = open;
    }

    /// The error banner below the top bar, with a button to close it.
    pub(super) fn error_banner(&mut self, ui: &mut egui::Ui) {
        let Some(err) = &self.error else { return };
        let mut close = false;
        let mut open_as = false;
        let can_retry = self.failed_file.is_some();
        ui.horizontal(|ui| {
            if ui.button("✕").on_hover_text("Close this message").clicked() {
                close = true;
            }
            if can_retry
                && ui
                    .button("Open as…")
                    .on_hover_text("Choose the format of this file yourself")
                    .clicked()
            {
                open_as = true;
            }
            ui.add(
                egui::Label::new(RichText::new(format!("⚠ {err}")).color(palette::error())).wrap(),
            );
        });
        if open_as && let Some((target, name, bytes)) = self.failed_file.take() {
            self.format_choice = Some(FormatChoice {
                candidates: crate::formats::detect(&bytes),
                target,
                name,
                bytes,
                reason: "Choose the format to read this file with.".to_owned(),
            });
            close = true;
        }
        if close || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.error = None;
        }
    }

    /// The "Which format is this file?" window: the likely formats first,
    /// then all the others; the chosen one reads the file.
    pub(super) fn format_window(&mut self, ctx: &egui::Context) {
        let Some(choice) = &self.format_choice else {
            return;
        };
        let mut chosen: Option<&'static dyn crate::formats::Format> = None;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::unique("format_choice")).show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.heading("Which format is this file?");
            ui.label(RichText::new(&choice.name).monospace());
            ui.label(&choice.reason);
            ui.add_space(6.0);
            let likely: Vec<&'static dyn crate::formats::Format> =
                choice.candidates.iter().map(|c| c.0).collect();
            if !likely.is_empty() {
                ui.label(RichText::new("Possible formats").strong());
                for (f, c) in &choice.candidates {
                    let how = match c {
                        crate::formats::Confidence::Sure => "certain",
                        crate::formats::Confidence::Likely => "likely",
                        _ => "possible",
                    };
                    if ui.button(format!("{} ({how})", f.name())).clicked() {
                        chosen = Some(*f);
                    }
                }
                ui.add_space(4.0);
            }
            let others: Vec<&'static dyn crate::formats::Format> = crate::formats::FORMATS
                .iter()
                .copied()
                .filter(|f| !likely.iter().any(|l| l.name() == f.name()))
                .collect();
            if !others.is_empty() {
                ui.label(RichText::new("Other formats").strong());
                for f in others {
                    if ui.button(f.name()).clicked() {
                        chosen = Some(f);
                    }
                }
            }
            ui.add_space(6.0);
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
        });
        if modal.should_close() {
            cancel = true;
        }
        if let Some(format) = chosen
            && let Some(choice) = self.format_choice.take()
        {
            self.read_as(choice.target, choice.name, &choice.bytes, format);
        } else if cancel {
            self.format_choice = None;
        }
    }

    /// The Print window: white (recommended) or black screen, then print.
    pub(super) fn print_window(&mut self, ctx: &egui::Context) {
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

    /// The statistics window (Measure menu → Statistics per cycle): one table
    /// per measured channel. A floating window, so that it never takes the
    /// space of the plot.
    pub(super) fn stats_window(&mut self, ctx: &egui::Context) {
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
                        let r = self.measure_cached(&name, &ch, &window);
                        let ch = &*ch;
                        any |= stats_table(ui, &name, &r, ch, color, &self.measure);
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
}

/// The contents of the Help window.
pub(super) fn help_text(ui: &mut egui::Ui) {
    let section = |ui: &mut egui::Ui, title: &str, lines: &[&str]| {
        ui.add_space(6.0);
        ui.label(RichText::new(title).strong().color(palette::fg()));
        for line in lines {
            ui.label(format!("• {line}"));
        }
    };
    ui.label(
        RichText::new(format!("WaveGlance {VERSION} · commit {GIT_HASH}"))
            .small()
            .color(palette::hint()),
    );
    section(
        ui,
        "Open a file",
        &[
            "Drop a file on the window, or use ⏏ File ▾ → Open file….",
            "File ▾ → Open example, or the welcome screen (button at the top of this window): real FNIRSI DPOX180H, 1013D and Rigol captures, and VCD files (a CPU bus, a DAC, UART, I2C and SPI) to try the viewer without a scope.",
            "Supported: FNIRSI DPOX180H, FNIRSI 1013D/1014D (original firmware), Keysight/Agilent .bin, Rigol .bin and .wfm (DS1000Z, DS1000E/D, DS2000), VCD (logic analyzers: digital signals and buses in lanes at the bottom), and CSV/text tables from any oscilloscope (Keysight, Rigol, Tektronix, Siglent… and this viewer's own export). The format is found automatically; the maker and model are shown at the top left.",
            "If the format is not certain, a window asks which one it is. If a file cannot be read, the red banner has an \"Open as…\" button to choose the format yourself.",
            "CSV: the first number column is the time (s), the others are channels; the names come from the header line. Files with up to millions of samples are drawn fast: only the part in view, at about two points per pixel.",
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
            "Trackpad: a vertical swipe zooms, a horizontal swipe moves along the time axis.",
            "Drag: move. Double-click: back to the start view.",
            "Touch: pinch to zoom, drag to move, double-tap to reset.",
            "⊕ Zoom: an overview of the whole record above the plot. The bright window is the part shown below. Click or drag in it to move there; wheel on it to zoom in time.",
            "⬌ Fit H: whole record. ⬍ Fit V: vertical axis fitted to what is on screen now. ⛶ Fit both: both. Scope screen: the time window the scope showed (a file opens on it when it stores it).",
            "Logic signals only (VCD): the axis is \"Logic\"; Y, Math, voltage cursors and voltage measurements are off; the FFT shows frequencies, with levels in dB relative to the peak.",
            "Buses (VCD): the value of each part is written in its lane. Right-click the bus readout at the bottom: Hex, Dec, Bin or ASCII. Hover a bus lane to read the value at the pointer.",
        ],
    );
    section(
        ui,
        "Vertical axis (Y: Volts / Div)",
        &[
            "Volts: one voltage axis for both channels.",
            "Div: like the scope screen, 8 divisions. Each channel uses its own V/div and starts from its ground marker.",
            "1▶ and 2▶ on the left edge: the 0 V level (ground) of each channel. Drag a marker to move that channel up or down. This needs Y: Div (each channel has its own scale); a drag in Volts mode switches to Div.",
            "Mouse wheel on the CH1 or CH2 readout (bottom bar): the V/div of that channel, in 1-2-5 steps, like the V/div knob (switches to Div). A * marks a V/div that is not the one in the file. Y ▾ → Reset channel positions and scales puts everything back.",
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
            "Draw: Line, or Bars (one bar per frequency bin, like a spectrum analyzer). Zoomed out, the bins in one screen column make one bar, at the highest level.",
            "Peak: the highest line above DC (white dot); its frequency is interpolated between bins. Δf: the frequency step; a longer time span gives a finer step.",
            "The FFT plot opens up to 20 × the peak frequency; ⛶ Fit (next to the readouts) or a double-click returns there; zoom out to see up to half the sample rate.",
        ],
    );
    section(
        ui,
        "Protocol decoder (⇄ Decode menu)",
        &[
            "UART: check each line to decode (a channel or a VCD signal); several lines can be decoded at the same time. The ▸ next to a line opens its own settings: Baud (Auto measures the rate from the shortest pulse), data bits, parity, stop bits and the idle level as the device sends them.",
            "An analog channel is made into 0 and 1 with a threshold (in its ▸ settings): Auto (the middle of the signal, with hysteresis) or a value in volts.",
            "The words are drawn just above the decoded signal and listed in a row under the plot: Hex, Dec, Bin or ASCII. Click a word in the row to centre it; ⎘ Copy copies all of them. A word with a parity or framing error is in the warning colour.",
            "I2C: check the SDA line, and in its ▸ settings choose I2C and the SCL line (a line named sda gets I2C with scl by itself). S = start, Sr = repeated start, W/R = address and direction, N = not acknowledged (NACK), P = stop; green and red dots mark start and stop.",
            "SPI: check the data line (MOSI or MISO, or both), and in its ▸ settings choose SPI, the SCLK line, the CS line (or None), the mode (0–3), the bit order and the word size. Lines named mosi/miso get SPI with sck and cs by themselves. [ and ] in the row: CS active and released. Without CS, a pause of the clock starts a new word.",
            "Use a time base that gives at least 4 samples per bit (10 or more is better); the row warns when there are fewer.",
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

/// The controls of the viewer, for the welcome screen.
fn controls_grid(ui: &mut egui::Ui) {
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
                (
                    "Drag 1▶ / 2▶",
                    "Move that channel up or down (switches Y to Div)",
                ),
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
}

/// The built-in examples as cards, by group (two columns when there is
/// room). Returns the example that was clicked.
fn example_cards(ui: &mut egui::Ui) -> Option<&'static examples::Example> {
    let mut clicked = None;
    let gap = 8.0;
    let width = ui.available_width();
    let columns = if width >= 520.0 { 2 } else { 1 };
    let card_width = (width - gap * (columns as f32 - 1.0)) / columns as f32;
    for g in examples::GROUPS {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(group_icon(g.name)).color(palette::hint()));
            ui.label(RichText::new(g.name).strong());
        });
        let items: Vec<&examples::Example> = examples::EXAMPLES
            .iter()
            .filter(|e| e.group == g.name)
            .collect();
        for row in items.chunks(columns) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for e in row {
                    if example_card(ui, e, card_width).clicked() {
                        clicked = Some(*e);
                    }
                }
            });
        }
        ui.label(RichText::new(g.credit).small().color(palette::hint()));
    }
    clicked
}

/// A mark for an example group: a waveform for scope captures, a logic
/// signal for the VCD files.
fn group_icon(group: &str) -> &'static str {
    if group.contains("VCD") { "⇄" } else { "∿" }
}

/// One example as a card: its name and file type; the description on
/// hover; it lights up under the pointer.
fn example_card(ui: &mut egui::Ui, e: &examples::Example, width: f32) -> egui::Response {
    let height = 30.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    let visuals = ui.visuals();
    let hovered = response.hovered();
    let (fill, stroke) = if hovered {
        (
            visuals.widgets.hovered.weak_bg_fill,
            egui::Stroke::new(1.0, visuals.selection.stroke.color),
        )
    } else {
        (
            visuals.faint_bg_color,
            visuals.widgets.noninteractive.bg_stroke,
        )
    };
    let painter = ui.painter();
    painter.rect(rect, 6.0, fill, stroke, egui::StrokeKind::Inside);
    let text_color = if hovered {
        visuals.strong_text_color()
    } else {
        visuals.text_color()
    };
    // The file type, as a small tag on the right.
    let ext = e.file.rsplit('.').next().unwrap_or("").to_uppercase();
    let tag_font = egui::FontId::monospace(10.0);
    let tag = painter.layout_no_wrap(ext, tag_font, palette::hint());
    let tag_rect = egui::Rect::from_min_size(
        egui::pos2(
            rect.right() - tag.size().x - 14.0,
            rect.center().y - tag.size().y / 2.0 - 2.0,
        ),
        tag.size() + egui::vec2(8.0, 4.0),
    );
    painter.rect_stroke(
        tag_rect,
        4.0,
        egui::Stroke::new(1.0, palette::hint().gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    painter.galley(tag_rect.min + egui::vec2(4.0, 2.0), tag, palette::hint());
    // The name, cut with … if it does not fit.
    let font = egui::FontId::proportional(13.5);
    let room = tag_rect.left() - rect.left() - 20.0;
    let mut job = egui::text::LayoutJob::simple_singleline(e.label.to_owned(), font, text_color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(room);
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    painter.galley(
        egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
        galley,
        text_color,
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.on_hover_text(format!("{}\n\nLink: {}", e.about, examples::link(e)))
}
