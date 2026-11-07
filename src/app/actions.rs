//! Export, screenshots, printing and the screen theme.

use super::*;

impl ScopeApp {
    /// Save the samples of the main waveform as CSV.
    pub(super) fn export_csv(&mut self) {
        let (Some(w), Some(name)) = (&self.waveform, &self.file_name) else {
            return;
        };
        let text = export::csv(w, name, VERSION);
        let file = format!("{}.csv", export::stem(name));
        if let Err(e) = export::save(&file, text.as_bytes(), "text/csv", ("CSV", "csv")) {
            self.error = Some(format!("Could not save {file}: {e}."));
        }
    }

    /// Start a screenshot (for a PNG file or for printing). Printing uses the
    /// white screen to save ink.
    /// `light`: the theme for the picture (`None`: the theme in use). The
    /// theme in use comes back after the picture.
    pub(super) fn start_shot(
        &mut self,
        ctx: &egui::Context,
        purpose: ShotPurpose,
        light: Option<bool>,
    ) {
        let restore_light = match light {
            Some(l) if l != self.light_theme => {
                let before = self.light_theme;
                self.set_light_theme(ctx, l);
                Some(before)
            }
            _ => None,
        };
        self.shot = Some(ShotJob {
            purpose,
            frames_left: 3,
            sent: false,
            restore_light,
        });
        ctx.request_repaint();
    }

    /// Advance the screenshot in progress: wait a few frames, send the
    /// command, then save or print the image when it arrives.
    pub(super) fn update_shot(&mut self, ctx: &egui::Context) {
        let Some(mut job) = self.shot else { return };
        if !job.sent {
            if job.frames_left > 0 {
                job.frames_left -= 1;
            } else {
                let tx = self.shot_tx.clone();
                let repaint = ctx.clone();
                ctx.request_screenshot(move |image| {
                    let _ = tx.send(image);
                    repaint.request_repaint();
                });
                job.sent = true;
            }
            self.shot = Some(job);
            ctx.request_repaint();
            return;
        }
        let Ok(image) = self.shot_rx.try_recv() else {
            return;
        };
        self.shot = None;
        if let Some(light) = job.restore_light {
            self.set_light_theme(ctx, light);
        }
        let base = self
            .file_name
            .as_deref()
            .map_or("fnirsi-viewer", export::stem);
        let result = export::png(&image).and_then(|png| match job.purpose {
            ShotPurpose::Save => export::save(
                &format!("{base}.png"),
                &png,
                "image/png",
                ("PNG image", "png"),
            )
            .map(|_| ()),
            ShotPurpose::Print => export::print_png(&png),
        });
        if let Err(e) = result {
            self.error = Some(format!("Could not save or print the picture: {e}."));
        }
    }

    /// Switch between the black and the white screen.
    pub(super) fn set_light_theme(&mut self, ctx: &egui::Context, light: bool) {
        self.light_theme = light;
        palette::set_light(light);
        ctx.set_theme(if light {
            egui::Theme::Light
        } else {
            egui::Theme::Dark
        });
    }
}
