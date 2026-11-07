//! FNIRSI oscilloscope waveform viewer (DPOX180H, 1013D, 1014D) (desktop and WebAssembly).

pub mod app;
pub mod cursors;
pub mod examples;
pub mod export;
pub mod fft;
pub mod formats;
pub mod measure;
pub mod model;
pub mod palette;

pub use app::ScopeApp;

/// Web entry point: starts the app on `<canvas id="the_canvas_id">`.
#[cfg(target_arch = "wasm32")]
mod web {
    use wasm_bindgen::JsCast as _;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(start)]
    pub fn start() {
        eframe::WebLogger::init(log::LevelFilter::Info).ok();
        wasm_bindgen_futures::spawn_local(async {
            let document = web_sys::window()
                .and_then(|w| w.document())
                .expect("no document");
            let canvas = document
                .get_element_by_id("the_canvas_id")
                .expect("no element with id 'the_canvas_id'")
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .expect("'the_canvas_id' is not a canvas");
            let result = eframe::WebRunner::new()
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(|cc| Ok(Box::new(super::ScopeApp::new(cc)))),
                )
                .await;
            if let Err(e) = result {
                log::error!("failed to start the app: {e:?}");
            }
        });
    }
}
