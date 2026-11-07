//! Desktop entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window on Windows

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("FNIRSI DPOX180H Viewer")
            .with_inner_size([1100.0, 700.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "FNIRSI DPOX180H Viewer",
        options,
        Box::new(|cc| Ok(Box::new(fnirsi_viewer::ScopeApp::new(cc)))),
    )
}

// The web build starts from `lib.rs` (`#[wasm_bindgen(start)]`).
#[cfg(target_arch = "wasm32")]
fn main() {}
