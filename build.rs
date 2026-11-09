//! Build script:
//! - the short git commit of the build, for the version label;
//! - on Windows, the icon and the version information of the desktop
//!   program (`waveglance-desktop.exe`), so Explorer, the taskbar and the
//!   file properties show them.

use std::process::Command;

fn main() {
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=GIT_HASH={hash}");
    // Run again after a commit or a branch change.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
    println!("cargo:rerun-if-changed=.git/packed-refs");
    println!("cargo:rerun-if-changed=assets/icon/windows/waveglance.ico");

    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon/windows/waveglance.ico")
            .set("ProductName", "WaveGlance")
            .set(
                "FileDescription",
                "WaveGlance: oscilloscope and logic analyzer file viewer",
            )
            .set("LegalCopyright", "MIT OR Apache-2.0");
        if let Err(e) = res.compile() {
            // A program without an icon still works: warn, do not fail;
            // but in GitHub Actions (CI is set) a release must have it.
            if std::env::var_os("CI").is_some() {
                panic!("could not add the Windows icon: {e}");
            }
            println!("cargo:warning=could not add the Windows icon: {e}");
        }
    }
}
