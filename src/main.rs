//! Desktop entry point.
//!
//! `waveglance-desktop [FILE]`: with a file name, the program opens that
//! file at start.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window on Windows

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();
    // The first argument is the file to open. macOS gives no argument when
    // a program starts from the Finder (it sends "-psn_…" on old versions).
    let file = std::env::args_os()
        .nth(1)
        .filter(|a| !a.to_string_lossy().starts_with("-psn_"))
        .map(std::path::PathBuf::from);
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon/icon-256.png"))
        .unwrap_or_default();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("WaveGlance")
            .with_inner_size([1100.0, 700.0])
            .with_icon(icon)
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "WaveGlance",
        options,
        Box::new(move |cc| {
            let mut app = waveglance::ScopeApp::new(cc);
            if let Some(file) = &file {
                app.open_path(file);
            }
            Ok(Box::new(app))
        }),
    )
}

// The web build starts from `lib.rs` (`#[wasm_bindgen(start)]`).
#[cfg(target_arch = "wasm32")]
fn main() {}
