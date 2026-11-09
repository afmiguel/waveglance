//! Keyboard, mouse and file input.

use super::*;

impl ScopeApp {
    /// Find the events again when the file, channel or kind changed.
    pub(super) fn update_search(&mut self) {
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
            .and_then(|w| w.traces.get(self.search.source))
            // A bus has values, not edges.
            .filter(|t| !t.is_bus())
            .map(|t| measure::find_events(t.points(), kind, MIN_AMPLITUDE_DIV * t.display.per_div))
            .unwrap_or_default();
    }

    /// Go to the next (`step` = 1) or previous (−1) event and centre the view on it.
    pub(super) fn search_step(&mut self, step: i64) {
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

    pub(super) fn receive_loaded_files(&mut self) {
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
    pub(super) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
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
            let result = read_file(file.path());
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

    /// Open a file given by its path, as the main waveform (desktop: the
    /// file named on the command line, or by "Open with" on Windows and
    /// Linux). Errors show in the banner, like for any other file.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_path(&mut self, path: &std::path::Path) {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let result = read_file(path);
        let _ = self.inbox_tx.send((LoadTarget::Main, name, result));
    }

    /// Show the "Open file" dialog (`rfd`) for the main waveform.
    pub(super) fn open_file_dialog(&mut self, ctx: &egui::Context) {
        self.open_dialog(ctx, LoadTarget::Main);
    }

    /// Show the "Open file" dialog (`rfd`); the file goes to `target`.
    pub(super) fn open_dialog(&mut self, ctx: &egui::Context, target: LoadTarget) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = ctx;
            let picked = rfd::FileDialog::new()
                .add_filter(
                    "Waveform (.wav, .bin, .wfm, .vcd, .csv)",
                    &[
                        "wav", "WAV", "bin", "BIN", "wfm", "WFM", "vcd", "VCD", "csv", "CSV",
                        "txt", "TXT",
                    ],
                )
                .add_filter("All files", &["*"])
                .pick_file();
            if let Some(path) = picked {
                let name = path
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                let result = read_file(&path);
                let _ = self.inbox_tx.send((target, name, result));
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let tx = self.inbox_tx.clone();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let picked = rfd::AsyncFileDialog::new()
                    .add_filter(
                        "Waveform (.wav, .bin, .wfm, .vcd, .csv)",
                        &[
                            "wav", "WAV", "bin", "BIN", "wfm", "WFM", "vcd", "VCD", "csv", "CSV",
                            "txt", "TXT",
                        ],
                    )
                    .add_filter("All files", &["*"])
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

    /// Div mode: start, follow and end the drag of a ground marker.
    pub(super) fn handle_marker_drag(&mut self, ctx: &egui::Context, on_plot: bool) {
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
            self.marker_drag = marker_at(&t, &self.marker_maps(), pos);
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
        let Some(ch) = self.traces().get(i) else {
            return;
        };
        let base = YMap::divisions(ch).offset;
        let y = t.value_from_position(pos).y;
        self.ground_shift[i] = y - base;
    }

    /// Keyboard shortcuts. Keys are taken unless a text field is typed in.
    pub(super) fn handle_keys(&mut self, ctx: &egui::Context) {
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
            let logic = self.logic_only();
            self.set_cursor_mode(match self.cursors.mode {
                CursorMode::Off => CursorMode::Time,
                // Logic only: Off and Time.
                CursorMode::Time if logic => CursorMode::Off,
                CursorMode::Time => CursorMode::Volt,
                CursorMode::Volt => CursorMode::Both,
                CursorMode::Both => CursorMode::Off,
            });
        }
        // 1 and 2: hide or show CH1 and CH2 (only the traces the file has).
        for (i, k) in [Key::Num1, Key::Num2].into_iter().enumerate() {
            if key(Modifiers::NONE, k)
                && let Some(v) = self.visible.get_mut(i)
            {
                *v = !*v;
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
            .traces()
            .first()
            .and_then(Trace::sample_interval)
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
}

/// Read a file on the desktop, after checking its size: a file over the
/// limit is refused before it is loaded into memory.
#[cfg(not(target_arch = "wasm32"))]
fn read_file(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    formats::check_size(usize::try_from(len).unwrap_or(usize::MAX)).map_err(|e| e.to_string())?;
    std::fs::read(path).map_err(|e| e.to_string())
}
