//! WaveGlance: a viewer for oscilloscope and logic analyzer files (FNIRSI,
//! Keysight, Rigol, VCD, CSV), with measurements, FFT and protocol decoders
//! (desktop and WebAssembly).

pub mod app;
pub mod cursors;
pub mod decode;
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
            let runner = eframe::WebRunner::new();
            show_a_message_on_panic();
            let result = runner
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

    /// In WebAssembly a panic stops the program and cannot be caught. Add a
    /// step to the panic hook (after the one of eframe) that shows the
    /// `#crash` box of `index.html`: what happened, the file being read,
    /// a "Reload" button and where to report it.
    fn show_a_message_on_panic() {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            previous(info);
            let Some(document) = web_sys::window().and_then(|w| w.document()) else {
                return;
            };
            let set = |id: &str, text: &str| {
                if let Some(e) = document.get_element_by_id(id) {
                    e.set_text_content(Some(text));
                }
            };
            if let Some(file) = crate::app::file_being_read() {
                set(
                    "crash_file",
                    &format!(
                        "It happened while reading {file}. The file may be damaged, \
                         or of a kind this viewer does not know."
                    ),
                );
            }
            set("crash_detail", &info.to_string());
            if let Some(e) = document.get_element_by_id("crash") {
                let _ = e.class_list().add_1("show");
            }
        }));
    }
}
