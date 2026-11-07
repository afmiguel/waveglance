//! The toolbar and its menus.

use super::*;

impl ScopeApp {
    /// Contents of the Math menu: the operation and its two traces.
    pub(super) fn math_menu(&mut self, ui: &mut egui::Ui) {
        let labels: Vec<String> = self.traces().iter().map(|t| t.label.clone()).collect();
        if labels.len() < 2 {
            ui.label("Needs two channels in the file.");
            return;
        }
        let mut m = self.math;
        for op in MathOp::ALL {
            let text = match op {
                MathOp::Off => "Off".to_owned(),
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
                        ui.radio_value(slot, i, RichText::new(label).color(palette::channel(i)));
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
            RichText::new("Sample by sample, in volts. Measured like a channel (MATH row).")
                .small()
                .color(palette::hint()),
        );
    }

    /// Contents of the Search menu.
    pub(super) fn search_menu(&mut self, ui: &mut egui::Ui) {
        let labels: Vec<String> = self.traces().iter().map(|t| t.label.clone()).collect();
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
            for (i, name) in labels.iter().enumerate() {
                if ui
                    .radio(
                        self.search.source == i,
                        RichText::new(name).color(palette::channel(i)),
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

    /// Contents of the Y menu: Volts (default) / Div, as radio buttons.
    pub(super) fn y_axis_buttons(&mut self, ui: &mut egui::Ui) {
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
        if self.ground_shift.iter().any(|&s| s != 0.0)
            || self.vdiv_override.iter().any(Option::is_some)
        {
            ui.separator();
            if ui
                .button("Reset channel positions and scales")
                .on_hover_text("Put the ground markers and the V/div back as the scope had them")
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
            .button("Open file…")
            .on_hover_text("Open a FNIRSI .wav file (Ctrl/⌘+O), or drop it on the window")
            .clicked()
        {
            ui.close();
            self.open_file_dialog(ui.ctx());
        }
        ui.menu_button("Open example", |ui| {
            for (k, g) in examples::GROUPS.iter().enumerate() {
                if k > 0 {
                    ui.separator();
                }
                ui.label(RichText::new(g.name).strong());
                for e in examples::EXAMPLES.iter().filter(|e| e.group == g.name) {
                    if ui.button(e.label).on_hover_text(e.about).clicked() {
                        self.load_bytes(e.file.to_owned(), e.bytes);
                        ui.close();
                    }
                }
                ui.label(RichText::new(g.credit).small().color(palette::hint()));
            }
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

    pub(super) fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            // Button text never wraps: a button that does not fit moves to the
            // next row as a whole.
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            // The model comes from the loaded file.
            let title = match &self.waveform {
                Some(w) => w.source.title(),
                None => "FNIRSI Viewer".to_owned(),
            };
            ui.label(RichText::new(title).strong().color(palette::fg_dim()))
                .on_hover_text("Reads FNIRSI DPOX180H and 1013D/1014D .wav, Keysight/Agilent and Rigol .bin, VCD, and CSV files of any oscilloscope");
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
            let math_label = if !self.math_available() {
                "Σ Math ▾".to_owned()
            } else {
                format!("Σ Math: {} ▾", self.math_label())
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
    pub(super) fn fit_buttons(&mut self, ui: &mut egui::Ui) {
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
    pub(super) fn cursor_menu(&mut self, ui: &mut egui::Ui) {
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

    /// Contents of the "Measure" menu.
    pub(super) fn measure_menu(&mut self, ui: &mut egui::Ui) {
        // The channels of the loaded file (none before a file is open).
        let labels: Vec<String> = self.traces().iter().map(|t| t.label.clone()).collect();
        let m = &mut self.measure;
        if !labels.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label("Channels:");
                for (i, name) in labels.iter().enumerate() {
                    let mut on = m.is_measured(name);
                    if ui
                        .checkbox(&mut on, RichText::new(name).color(palette::channel(i)))
                        .changed()
                    {
                        m.set_measured(name, on);
                    }
                }
            });
        }
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

    /// Contents of the FFT menu.
    pub(super) fn fft_menu(&mut self, ui: &mut egui::Ui) {
        let labels: Vec<String> = self.traces().iter().map(|t| t.label.clone()).collect();
        ui.checkbox(&mut self.fft.show, "Show FFT");
        ui.separator();
        ui.label("Source:");
        ui.horizontal_wrapped(|ui| {
            for (i, name) in labels.iter().enumerate() {
                if ui
                    .radio(
                        self.fft.source == i,
                        RichText::new(name).color(palette::channel(i)),
                    )
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
pub(super) fn cursor_mode_name(mode: CursorMode) -> &'static str {
    match mode {
        CursorMode::Off => "Off",
        CursorMode::Time => "Time",
        CursorMode::Volt => "Volt",
        CursorMode::Both => "Both",
    }
}
