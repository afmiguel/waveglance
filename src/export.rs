//! Export: the samples as CSV, a screenshot as PNG, and saving a file
//! (a save dialog on the desktop, a download in the browser).

use crate::model::{Acquisition, Capture};
use eframe::egui::ColorImage;
use std::fmt::Write as _;

/// The samples of a capture as CSV: a few `#` comment lines with the
/// settings, a header line, then one line per sample: time (s) and the
/// value of each trace (traces with the same sample times share the lines).
pub fn csv(w: &Capture, file_name: &str, app_version: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# FNIRSI Viewer {app_version}");
    let _ = writeln!(out, "# file: {file_name}; source: {}", w.source.title());
    if let Some(t) = w.time_per_div {
        let _ = writeln!(out, "# time/div: {} s", num(t));
    }
    match w.acquisition {
        Acquisition::RealTime { sample_rate_hz } => {
            let _ = writeln!(
                out,
                "# sample rate: {} Sa/s; t = 0 is the trigger",
                num(sample_rate_hz)
            );
        }
        Acquisition::Equivalent { frames } => {
            let _ = writeln!(
                out,
                "# equivalent-time sampling: frame 1 of {frames}; t = 0 is the trigger"
            );
        }
    }
    for t in &w.traces {
        let coupling = t
            .display
            .coupling
            .map_or(String::new(), |c| format!(", {}", c.name()));
        let _ = writeln!(
            out,
            "# {}: {} {}/div, probe {}X{coupling}",
            t.label,
            num(t.display.per_div),
            t.unit.symbol(),
            num(t.display.probe)
        );
    }
    out.push_str("time_s");
    for t in &w.traces {
        let _ = write!(out, ",{}_{}", t.label.to_lowercase(), t.unit.symbol());
    }
    out.push('\n');
    let points: Vec<&[[f64; 2]]> = w.traces.iter().map(|t| t.points()).collect();
    let rows = points.iter().map(|p| p.len()).max().unwrap_or(0);
    for i in 0..rows {
        let Some(t) = points.iter().find_map(|p| p.get(i).map(|q| q[0])) else {
            continue;
        };
        out.push_str(&num(t));
        for p in &points {
            match p.get(i) {
                Some(q) => {
                    let _ = write!(out, ",{}", num(q[1]));
                }
                None => out.push(','),
            }
        }
        out.push('\n');
    }
    out
}

/// A number with at most 10 significant digits and no trailing zeros, so
/// that binary rounding noise (7.500000000000001e-3) does not show.
/// Plain notation from 1e-3 to 1e7, else scientific (-7.5e-6).
pub fn num(v: f64) -> String {
    if v == 0.0 || !v.is_finite() {
        return format!("{v}");
    }
    let rounded: f64 = format!("{v:.9e}").parse().unwrap_or(v);
    if (1e-3..1e7).contains(&rounded.abs()) {
        format!("{rounded}")
    } else {
        format!("{rounded:e}")
    }
}

/// Encode a screenshot as PNG.
pub fn png(image: &ColorImage) -> Result<Vec<u8>, String> {
    let [w, h] = image.size;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for c in &image.pixels {
        rgba.extend_from_slice(&c.to_srgba_unmultiplied());
    }
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&rgba).map_err(|e| e.to_string())?;
    }
    Ok(bytes)
}

/// The base of a file name without its extension ("1.wav" → "1").
pub fn stem(file_name: &str) -> &str {
    file_name.rsplit_once('.').map_or(file_name, |(s, _)| s)
}

/// Save `bytes` as a file. Desktop: a save dialog. Browser: a download.
/// Returns `Ok(false)` if the user closed the dialog without saving.
pub fn save(
    file_name: &str,
    bytes: &[u8],
    mime: &str,
    filter: (&str, &str),
) -> Result<bool, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = mime;
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(file_name)
            .add_filter(filter.0, &[filter.1])
            .save_file()
        else {
            return Ok(false);
        };
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(true)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = filter;
        download(file_name, bytes, mime)?;
        Ok(true)
    }
}

/// Print a PNG picture.
/// Browser: the picture is put in the page (shown only when printing) and
/// the print dialog opens. Desktop: the picture opens in the system image
/// viewer, which can print it.
pub fn print_png(png: &[u8]) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = std::env::temp_dir().join("fnirsi-viewer-print.png");
        std::fs::write(&path, png).map_err(|e| format!("{}: {e}", path.display()))?;
        #[cfg(target_os = "macos")]
        let mut command = std::process::Command::new("open");
        #[cfg(target_os = "windows")]
        let mut command = {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", "start", ""]);
            c
        };
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let mut command = std::process::Command::new("xdg-open");
        command
            .arg(&path)
            .spawn()
            .map_err(|e| format!("cannot open the image viewer: {e}"))?;
        Ok(())
    }
    #[cfg(target_arch = "wasm32")]
    {
        print_in_browser(png)
    }
}

/// Put the picture in `<img id="print_image">`, mark the page as printing
/// (index.html hides the canvas and shows the picture for print), and open
/// the print dialog when the picture is loaded.
#[cfg(target_arch = "wasm32")]
fn print_in_browser(png: &[u8]) -> Result<(), String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let array = js_sys::Uint8Array::from(png);
    let parts = js_sys::Array::new();
    parts.push(&array.buffer());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("image/png");
    let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&parts, &options)
        .map_err(err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let body = document.body().ok_or("no body")?;
    let image: web_sys::HtmlImageElement = document
        .get_element_by_id("print_image")
        .ok_or("no print_image element in the page")?
        .dyn_into()
        .map_err(|_| "print_image is not an image".to_owned())?;
    body.class_list().add_1("printing").map_err(err)?;
    let src = url.clone();
    let on_load = Closure::once_into_js(move || {
        // print() returns when the print dialog closes.
        let _ = window.print();
        let _ = body.class_list().remove_1("printing");
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    image.set_onload(Some(on_load.unchecked_ref()));
    image.set_src(&src);
    Ok(())
}

/// Make the browser download `bytes` as `file_name`.
#[cfg(target_arch = "wasm32")]
fn download(file_name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
    use wasm_bindgen::JsCast;
    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array.buffer());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&parts, &options)
        .map_err(err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("no document")?;
    let anchor: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(err)?
        .dyn_into()
        .map_err(|_| "not an anchor element".to_owned())?;
    anchor.set_href(&url);
    anchor.set_download(file_name);
    anchor.click();
    web_sys::Url::revoke_object_url(&url).map_err(err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Color32;

    #[test]
    fn csv_of_an_example() {
        let e = &crate::examples::EXAMPLES[0];
        let w = crate::formats::open(e.bytes).unwrap();
        let text = csv(&w, e.file, "0.0.0");
        let lines: Vec<&str> = text.lines().collect();
        let header = lines.iter().position(|l| !l.starts_with('#')).unwrap();
        assert_eq!(lines[header], "time_s,ch1_V,ch2_V");
        assert_eq!(lines.len() - header - 1, 1500, "one line per sample");
        let first: Vec<f64> = lines[header + 1]
            .split(',')
            .map(|v| v.parse().unwrap())
            .collect();
        assert_eq!(first.len(), 3);
        assert!(
            (first[0] + 7.5e-3).abs() < 1e-9,
            "starts 15 divisions before the trigger"
        );
        assert!(
            lines[header + 1].starts_with("-0.0075,"),
            "{}",
            lines[header + 1]
        );
        assert!(text.contains("# sample rate: 100000 Sa/s"), "{text}");
    }

    #[test]
    fn numbers_without_rounding_noise() {
        assert_eq!(num(-7.500000000000001e-3), "-0.0075");
        assert_eq!(num(99999.99999999999), "100000");
        assert_eq!(num(-7.5e-6), "-7.5e-6");
        assert_eq!(num(0.0), "0");
        assert_eq!(num(1.28), "1.28");
    }

    #[test]
    fn png_header() {
        let image = ColorImage::new([4, 3], vec![Color32::RED; 12]);
        let bytes = png(&image).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(stem("1.wav"), "1");
        assert_eq!(stem("noext"), "noext");
    }
}
