//! The toolbar and its menus.

use super::*;

impl ScopeApp {
    /// Contents of the Math menu: the operation and its two traces.
    pub(super) fn math_menu(&mut self, ui: &mut egui::Ui) {
        let labels: Vec<String> = self.traces().iter().map(|t| t.label.clone()).collect();
        if labels.len() < 2 {
            ui.label(tr("Needs two channels in the file."));
            return;
        }
        let buses = self.bus_flags();
        let mut m = self.math;
        for op in MathOp::ALL {
            let text = match op {
                MathOp::Off => tr("Off").to_owned(),
                _ => format!("A {} B", op.symbol()),
            };
            ui.radio_value(&mut m.op, op, text);
        }
        if m.op != MathOp::Off {
            ui.separator();
            for (name, which) in [("A:", 0), ("B:", 1)] {
                ui.horizontal(|ui| {
                    ui.label(name);
                    for (i, label) in labels.iter().enumerate() {
                        let slot = if which == 0 { &mut m.a } else { &mut m.b };
                        let r = ui
                            .add_enabled(
                                !buses[i],
                                egui::RadioButton::new(
                                    *slot == i,
                                    RichText::new(label).color(palette::channel(i)),
                                ),
                            )
                            .on_disabled_hover_text(BUS_NOT_HERE);
                        if r.clicked() {
                            *slot = i;
                        }
                    }
                });
            }
        }
        if m != self.math {
            self.math = m;
            self.math_visible = true;
        }
        ui.separator();
        ui.label(
            RichText::new(tr("Sample by sample, in volts, at the times of A (B is interpolated when its samples are at other times). Measured like a channel (MATH row)."))
                .small()
                .color(palette::hint()),
        );
    }

    /// Contents of the Search menu.
    pub(super) fn search_menu(&mut self, ui: &mut egui::Ui) {
        let labels: Vec<String> = self.traces().iter().map(|t| t.label.clone()).collect();
        let buses = self.bus_flags();
        let mut kind = self.search.kind;
        ui.radio_value(&mut kind, None, tr("Off"));
        for k in measure::EventKind::ALL {
            ui.radio_value(&mut kind, Some(k), k.label()).on_hover_text(match k {
                measure::EventKind::Glitch => {
                    tr("Pulses much shorter than the others (shorter than 10% of the typical time between edges)")
                }
                _ => tr("At the 50% level between Vbase and Vtop, with hysteresis (10%–90%)"),
            });
        }
        if kind != self.search.kind {
            self.search.kind = kind;
            self.search.current = None;
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(tr("In:"));
            for (i, name) in labels.iter().enumerate() {
                if ui
                    .add_enabled(
                        !buses[i],
                        egui::RadioButton::new(
                            self.search.source == i,
                            RichText::new(name).color(palette::channel(i)),
                        ),
                    )
                    .on_disabled_hover_text(BUS_NOT_HERE)
                    .clicked()
                {
                    self.search.source = i;
                    self.search.current = None;
                }
            }
        });
        if self.search.kind.is_some() {
            ui.separator();
            ui.label(crate::trf!(
                "{a0} found in the whole record",
                a0 = self.search.events.len()
            ));
            ui.horizontal(|ui| {
                if ui
                    .button(tr("◀ Previous"))
                    .on_hover_text(tr("Key ["))
                    .clicked()
                {
                    self.search_step(-1);
                }
                if ui.button(tr("Next ▶")).on_hover_text(tr("Key ]")).clicked() {
                    self.search_step(1);
                }
            });
        }
    }

    /// Contents of the Y menu: Volts (default) / Div, as radio buttons.
    pub(super) fn y_axis_buttons(&mut self, ui: &mut egui::Ui) {
        let mut y_axis = self.y_axis;
        ui.radio_value(&mut y_axis, YAxis::Volts, tr("Volts"))
            .on_hover_text(tr("Vertical axis in volts: both channels share the axis"));
        ui.radio_value(&mut y_axis, YAxis::Divisions, tr("Div")).on_hover_text(
            tr("Vertical axis in divisions, like the scope screen: each channel has its own V/div and ground marker"),
        );
        if y_axis != self.y_axis {
            self.set_y_axis(y_axis);
            ui.close();
        }
        if self.ground_shift.iter().any(|&s| s != 0.0)
            || self.vdiv_override.iter().any(Option::is_some)
        {
            ui.separator();
            if ui
                .button(tr("Reset channel positions and scales"))
                .on_hover_text(tr(
                    "Put the ground markers and the V/div back as the scope had them",
                ))
                .clicked()
            {
                self.ground_shift.iter_mut().for_each(|s| *s = 0.0);
                self.vdiv_override.iter_mut().for_each(|v| *v = None);
                ui.close();
            }
        }
    }

    /// Contents of the File menu.
    pub(super) fn file_menu(&mut self, ui: &mut egui::Ui) {
        let loaded = self.waveform.is_some();
        if ui
            .button(tr("Open file…"))
            .on_hover_text(tr("Open a file (Ctrl/⌘+O), or drop it on the window"))
            .clicked()
        {
            ui.close();
            self.open_file_dialog(ui.ctx());
        }
        ui.menu_button(tr("Open example"), |ui| {
            for (k, g) in examples::GROUPS.iter().enumerate() {
                if k > 0 {
                    ui.separator();
                }
                ui.label(RichText::new(tr(g.name)).strong());
                for e in examples::EXAMPLES.iter().filter(|e| e.group == g.name) {
                    if ui.button(tr(e.label)).on_hover_text(tr(e.about)).clicked() {
                        self.load_bytes(e.file.to_owned(), e.bytes);
                        ui.close();
                    }
                }
                ui.label(RichText::new(tr(g.credit)).small().color(palette::hint()));
            }
        });
        ui.separator();
        if ui
            .add_enabled(loaded, egui::Button::new(tr("Open reference…")))
            .on_hover_text(tr(
                "Open a second file, drawn faint behind the traces, to compare with",
            ))
            .clicked()
        {
            ui.close();
            self.open_dialog(ui.ctx(), LoadTarget::Reference);
        }
        if let Some((name, _)) = &self.reference {
            let label = crate::trf!("Show reference ({name})", name = name);
            ui.checkbox(&mut self.show_reference, label);
            if ui.button(tr("Close reference")).clicked() {
                self.reference = None;
                ui.close();
            }
        }
        ui.separator();
        if ui
            .add_enabled(loaded, egui::Button::new(tr("Export CSV…")))
            .on_hover_text(tr(
                "Save the samples as a table: time and the voltage of each channel",
            ))
            .clicked()
        {
            ui.close();
            self.export_csv();
        }
        if ui
            .add_enabled(loaded, egui::Button::new(tr("Save screenshot (PNG)…")))
            .on_hover_text(tr("Save a picture of the plots and readouts, as shown"))
            .clicked()
        {
            ui.close();
            self.start_shot(ui.ctx(), ShotPurpose::Save, None);
        }
        if ui
            .add_enabled(loaded, egui::Button::new(tr("Print…")))
            .on_hover_text(tr(
                "Print the plots and readouts on a white screen (Ctrl/⌘+P)",
            ))
            .clicked()
        {
            ui.close();
            self.print_dialog = true;
        }
    }

    pub(super) fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            // Button text never wraps: a button that does not fit moves to the
            // next row as a whole.
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            // The model comes from the loaded file.
            let title = match &self.waveform {
                Some(w) => w.source.title(),
                None => tr("WaveGlance").to_owned(),
            };
            ui.label(RichText::new(title).strong().color(palette::fg_dim()))
                .on_hover_text(tr("Reads FNIRSI DPOX180H and 1013D/1014D .wav, Keysight/Agilent .bin, Rigol .bin and .wfm, Digilent WaveForms .csv and .txt, VCD, and CSV files of any oscilloscope"));
            // Version, discreet: identifies the build (also between releases).
            ui.label(
                RichText::new(format!("v{VERSION}"))
                    .small()
                    .color(palette::hint()),
            )
            .on_hover_text(crate::trf!("WaveGlance {VERSION}, commit {GIT_HASH}", VERSION = VERSION, GIT_HASH = GIT_HASH));
            ui.add_space(8.0);
            ui.menu_button(tr("⏏ File ▾"), |ui| {
                ui.style_mut().wrap_mode = None;
                self.file_menu(ui);
            })
            .response
            .on_hover_text(tr("Open, examples, reference, export, print"));
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
                tr("⛶ Fit ▾").to_owned(),
                tr("Fit the view to the waveform"),
                None,
                |ui| self.fit_buttons(ui),
            );
            ui.add_space(8.0);
            let y_label = match self.y_axis {
                YAxis::Volts => tr("Y: Volts ▾"),
                YAxis::Divisions => tr("Y: Div ▾"),
            };
            let logic = self.logic_only();
            if logic {
                logic_disabled(ui, tr("Y: Logic"), tr("the lanes have no voltage axis"));
            } else {
                toolbar_menu(
                    ui,
                    true,
                    y_label.to_owned(),
                    tr("Vertical axis: volts, or divisions like the scope screen"),
                    None,
                    |ui| self.y_axis_buttons(ui),
                );
            }
            ui.add_space(8.0);
            if ui
                .add_enabled(loaded, egui::Button::selectable(self.zoom_view, tr("⊕ Zoom")))
                .on_hover_text(
                    tr("Zoom view: an overview of the whole record above the plot. \
                     Click or drag in it to move, wheel to zoom."),
                )
                .clicked()
            {
                self.zoom_view = !self.zoom_view;
            }
            ui.add_space(8.0);
            let fft_label = if self.fft.show {
                let name = self
                    .fft_source()
                    .and_then(|i| self.traces().get(i))
                    .map_or("CH1".to_owned(), |t| t.label.clone());
                format!("∿ FFT: {name} ▾")
            } else {
                "∿ FFT ▾".to_owned()
            };
            toolbar_menu(
                ui,
                loaded,
                fft_label,
                tr("Spectrum of a channel (FFT) under the plot"),
                stay_open(),
                |ui| self.fft_menu(ui),
            );
            ui.add_space(8.0);
            // Cursor menu: the button shows the mode in use.
            let cursor_label = match self.cursors.mode {
                CursorMode::Off => tr("⌖ Cursors ▾").to_owned(),
                m => crate::trf!("⌖ Cursors: {a0} ▾", a0 = cursor_mode_name(m)),
            };
            toolbar_menu(
                ui,
                loaded,
                cursor_label,
                tr("Measurement cursors"),
                None,
                |ui| self.cursor_menu(ui),
            );
            ui.add_space(8.0);
            // Always enabled: the choice can be made before a file is loaded.
            // Stay open while items are ticked; close on a click outside the menu.
            toolbar_menu(
                ui,
                true,
                tr("▤ Measure ▾").to_owned(),
                tr("Choose the automatic measurements to show"),
                stay_open(),
                |ui| self.measure_menu(ui),
            );
            ui.add_space(8.0);
            let math_label = if !self.math_available() {
                tr("Σ Math ▾").to_owned()
            } else {
                crate::trf!("Σ Math: {a0} ▾", a0 = self.math_label())
            };
            if logic {
                logic_disabled(ui, tr("Σ Math ▾"), tr("the Math operations work on voltages"));
            } else {
                toolbar_menu(
                    ui,
                    loaded,
                    math_label,
                    tr("A channel computed from CH1 and CH2"),
                    None,
                    |ui| self.math_menu(ui),
                );
            }
            ui.add_space(8.0);
            let search_label = match self.search.kind {
                Some(k) => crate::trf!("⌕ Search: {a0} ▾", a0 = k.label()),
                None => tr("⌕ Search ▾").to_owned(),
            };
            toolbar_menu(
                ui,
                loaded,
                search_label,
                tr("Find edges or glitches in the whole record"),
                stay_open(),
                |ui| self.search_menu(ui),
            );
            ui.add_space(8.0);
            let decoded = self.decoded_lines();
            // The count only: a name would change the width of the button,
            // and the open menu would jump when a line is checked.
            let decode_label = match decoded.len() {
                0 => tr("⇄ Decode ▾").to_owned(),
                n => crate::trf!("⇄ Decode: ×{n} ▾", n = n),
            };
            toolbar_menu(
                ui,
                loaded,
                decode_label,
                tr("Protocol decoder (logic analyzer): serial data as bytes"),
                stay_open(),
                |ui| self.decode_menu(ui),
            );
            ui.add_space(8.0);
            let (theme_label, theme_tip) = if self.light_theme {
                (tr("◐ Black"), tr("Black screen, like the scope"))
            } else {
                (tr("◐ White"), tr("White screen with darker traces, for printing"))
            };
            if ui.button(theme_label).on_hover_text(theme_tip).clicked() {
                self.set_light_theme(ui.ctx(), !self.light_theme);
            }
            ui.add_space(8.0);
            let lang = crate::i18n::current();
            ui.menu_button(format!("🌐 {} ▾", lang.code()), |ui| {
                for l in crate::i18n::Lang::ALL {
                    if ui.radio(lang == l, l.name()).clicked() {
                        crate::i18n::set(l);
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text(tr("Language of the interface"));
            ui.add_space(8.0);
            if ui
                .button(tr("? Help"))
                .on_hover_text(tr("How to use the viewer: controls, cursors, measurements"))
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
    pub(super) fn fit_buttons(&mut self, ui: &mut egui::Ui) {
        for (label, fit, tip) in [
            (tr("⬌ Fit H"), Fit::Horizontal, tr("Show the whole record")),
            (
                tr("⬍ Fit V"),
                Fit::Vertical,
                tr("Fit the voltage axis to the samples between the first and last instant shown"),
            ),
            (
                tr("⛶ Fit both"),
                Fit::Both,
                tr("Full time span, then fit the voltage axis"),
            ),
        ] {
            if ui.button(label).on_hover_text(tip).clicked() {
                self.pending_fit = Some(fit);
                ui.close();
            }
        }
        if ui
            .add_enabled(
                self.screen_window().is_some(),
                egui::Button::new(tr("Scope screen")),
            )
            .on_hover_text(tr(
                "Show the time window the scope showed on its screen (the file opens on it). \
                 Fit H shows the whole record.",
            ))
            .on_disabled_hover_text(tr("This file does not store the screen window"))
            .clicked()
        {
            self.pending_fit = Some(Fit::Screen);
            ui.close();
        }
    }

    /// Contents of the "Cursors" menu: the mode (radio buttons) and an
    /// action to bring the cursors back into view.
    pub(super) fn cursor_menu(&mut self, ui: &mut egui::Ui) {
        let mut mode = self.cursors.mode;
        for (m, tip) in [
            (CursorMode::Off, tr("No cursors")),
            (
                CursorMode::Time,
                tr("Time cursors T1 and T2 (vertical lines)"),
            ),
            (
                CursorMode::Volt,
                tr("Voltage cursors V1 and V2 (horizontal lines)"),
            ),
            (CursorMode::Both, tr("Time and voltage cursors")),
        ] {
            // Logic only: voltage cursors measure nothing.
            let enabled = !(self.logic_only() && m.shows_volt());
            ui.add_enabled_ui(enabled, |ui| {
                ui.radio_value(&mut mode, m, cursor_mode_name(m))
                    .on_hover_text(tip)
                    .on_disabled_hover_text(tr(
                        "Only logic signals in this file: voltage cursors measure nothing",
                    ));
            });
        }
        if mode != self.cursors.mode {
            self.set_cursor_mode(mode);
            ui.close();
        }
        ui.separator();
        if ui
            .add_enabled(
                mode != CursorMode::Off,
                egui::Button::new(tr("Bring to view")),
            )
            .on_hover_text(tr("Move the cursors to 1/4 and 3/4 of the screen"))
            .clicked()
        {
            self.cursors.bring_to_view();
            ui.close();
        }
        ui.label(
            RichText::new(tr("Drag a dashed cursor line to move it."))
                .small()
                .color(palette::hint()),
        );
    }

    /// Contents of the "Measure" menu.
    pub(super) fn measure_menu(&mut self, ui: &mut egui::Ui) {
        // The channels of the loaded file (none before a file is open).
        let labels: Vec<String> = self.traces().iter().map(|t| t.label.clone()).collect();
        let logic = self.logic_only();
        let buses = self.bus_flags();
        let m = &mut self.measure;
        if !labels.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Channels:"));
                for (i, name) in labels.iter().enumerate() {
                    let mut on = m.is_measured(name) && !buses[i];
                    if ui
                        .add_enabled(
                            !buses[i],
                            egui::Checkbox::new(
                                &mut on,
                                RichText::new(name).color(palette::channel(i)),
                            ),
                        )
                        .on_disabled_hover_text(BUS_NOT_HERE)
                        .changed()
                    {
                        m.set_measured(name, on);
                    }
                }
            });
        }
        ui.checkbox(&mut m.stats, tr("Statistics per cycle"))
            .on_hover_text(tr(
                "A window with mean, min, max, σ and number of cycles of the time measurements",
            ));
        ui.horizontal(|ui| {
            ui.label(tr("Samples:"));
            for w in measure::Window::ALL {
                let tip = match w {
                    measure::Window::Screen => tr("The time span shown now"),
                    measure::Window::Record => tr("All the samples in the file"),
                    measure::Window::Cursors => {
                        tr("Between the time cursors (the screen if they are off)")
                    }
                };
                ui.selectable_value(&mut m.window, w, w.label())
                    .on_hover_text(tip);
            }
        });
        ui.separator();
        for item in Measurement::ALL {
            let mut on = m.is_selected(item);
            // Logic signals: only time measurements mean something.
            let enabled = !(logic && !item.applies_to_logic());
            let r = ui
                .add_enabled(enabled, egui::Checkbox::new(&mut on, item.label()))
                .on_hover_text(item.description())
                .on_disabled_hover_text(crate::trf!(
                    "{a0}\nNot for logic signals: {a1}",
                    a0 = item.description(),
                    a1 = if item.unit() == measure::Unit::Volt {
                        "the level is only 0 or 1"
                    } else {
                        tr("the steps are ideal")
                    }
                ));
            if r.changed() {
                m.set_selected(item, on);
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(tr("All")).clicked() {
                m.selected = Measurement::ALL.to_vec();
            }
            if ui.button(tr("None")).clicked() {
                m.selected.clear();
            }
            if ui
                .button(tr("Default"))
                .on_hover_text(tr("Freq and Vpp, both channels, screen samples"))
                .clicked()
            {
                *m = measure::Settings::default();
            }
        });
    }

    /// Contents of the FFT menu.
    pub(super) fn fft_menu(&mut self, ui: &mut egui::Ui) {
        let labels: Vec<String> = self.traces().iter().map(|t| t.label.clone()).collect();
        ui.checkbox(&mut self.fft.show, tr("Show FFT"));
        ui.separator();
        ui.label(tr("Source:"));
        let buses = self.bus_flags();
        let source = self.fft_source();
        ui.horizontal_wrapped(|ui| {
            for (i, name) in labels.iter().enumerate() {
                if ui
                    .add_enabled(
                        !buses[i],
                        egui::RadioButton::new(
                            source == Some(i),
                            RichText::new(name).color(palette::channel(i)),
                        ),
                    )
                    .on_disabled_hover_text(BUS_NOT_HERE)
                    .clicked()
                {
                    self.fft.source = i;
                }
            }
        });
        ui.label(tr("Window:"));
        for w in fft::Window::ALL {
            ui.radio_value(&mut self.fft.window, w, w.label())
                .on_hover_text(w.description());
        }
        ui.label(tr("Scale:"));
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.fft.db, true, tr("dBV"))
                .on_hover_text(tr("Decibels relative to 1 V RMS"));
            ui.radio_value(&mut self.fft.db, false, "V RMS");
        });
        ui.label(tr("Draw:"));
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.fft.bars, false, tr("Line"))
                .on_hover_text(tr("A line through the levels of the frequency bins"));
            ui.radio_value(&mut self.fft.bars, true, tr("Bars")).on_hover_text(
                tr("One bar per frequency bin, like a spectrum analyzer. Zoomed out, the bins in one screen column make one bar (the highest level)"),
            );
        });
        ui.separator();
        ui.label(
            RichText::new(tr("The FFT uses the samples shown in the time plot."))
                .small()
                .color(palette::hint()),
        );
    }
}

/// Hover text of a bus in a list of channels where buses do not apply.
const BUS_NOT_HERE: &str = "A bus has values, not a waveform: not for this";

impl ScopeApp {
    /// For each trace: true if it is a bus.
    pub(super) fn bus_flags(&self) -> Vec<bool> {
        self.traces().iter().map(Trace::is_bus).collect()
    }
}

/// A menu button of the toolbar, or the same button disabled.
pub(super) fn toolbar_menu(
    ui: &mut egui::Ui,
    enabled: bool,
    label: String,
    tip: &str,
    config: Option<egui::containers::menu::MenuConfig>,
    contents: impl FnOnce(&mut egui::Ui),
) {
    if !enabled {
        ui.add_enabled(false, egui::Button::new(label))
            .on_disabled_hover_text(crate::trf!("{tip} (open a file first)", tip = tip));
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

/// A toolbar button that is off because the file has only logic signals.
fn logic_disabled(ui: &mut egui::Ui, label: &str, why: &str) {
    ui.add_enabled(false, egui::Button::new(label))
        .on_disabled_hover_text(crate::trf!(
            "Only logic signals in this file: {why}",
            why = why
        ));
}

/// Name of a cursor mode, for the menu.
pub(super) fn cursor_mode_name(mode: CursorMode) -> &'static str {
    match mode {
        CursorMode::Off => tr("Off"),
        CursorMode::Time => tr("Time"),
        CursorMode::Volt => tr("Volt"),
        CursorMode::Both => tr("Both"),
    }
}
