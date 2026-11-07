//! Example files built into the app (File → Open example).
//!
//! FNIRSI 1013D captures by lexus2026HA, CC BY-NC 4.0: see
//! `assets/examples/1013d/README.md`. There are no DPOX180H examples yet: the
//! only public DPOX180H files have no license.

/// One built-in example file.
pub struct Example {
    /// Menu label.
    pub label: &'static str,
    /// What the example shows (menu tooltip).
    pub about: &'static str,
    /// File name shown after loading.
    pub file: &'static str,
    pub bytes: &'static [u8],
}

/// Credit for the example files, shown in the menu.
pub const CREDIT: &str = "FNIRSI 1013D captures by lexus2026HA \
    (github.com/lexus2026HA/FNIRSI-2C53T-Decoder), license CC BY-NC 4.0";

pub const EXAMPLES: &[Example] = &[
    Example {
        label: "1 kHz square wave, CH1 and CH2",
        about: "1013D, 500 µs/div. CH1 1 V/div DC, about 3.2 Vpp. CH2 is on with no signal. \
                Try the measurements and the FFT (odd harmonics).",
        file: "example-1013d-1khz-square.wav",
        bytes: include_bytes!("../assets/examples/1013d/1.wav"),
    },
    Example {
        label: "1 kHz square wave, clipped",
        about: "1013D, 100 µs/div, CH1 0.5 V/div. The signal goes past the bottom of the \
                screen: the voltage measurements are marked with \"?\".",
        file: "example-1013d-clipped.wav",
        bytes: include_bytes!("../assets/examples/1013d/2.wav"),
    },
    Example {
        label: "Small AC signal, 50 mV/div",
        about: "1013D, 2 ms/div, AC coupling, about 36 mVpp: a low-level, noisy signal.",
        file: "example-1013d-small-ac.wav",
        bytes: include_bytes!("../assets/examples/1013d/3.wav"),
    },
    Example {
        label: "1 MHz signal",
        about: "1013D, 500 ns/div, CH1 1 V/div AC, about 5 Vpp.",
        file: "example-1013d-1mhz.wav",
        bytes: include_bytes!("../assets/examples/1013d/8.wav"),
    },
    Example {
        label: "10 MHz signal",
        about: "1013D, 100 ns/div, CH1 1 V/div AC: few samples per period.",
        file: "example-1013d-10mhz.wav",
        bytes: include_bytes!("../assets/examples/1013d/7.wav"),
    },
    Example {
        label: "500 kHz on two channels",
        about: "1013D, 1 µs/div. CH1 1 V/div and CH2 2.5 V/div, both AC. \
                Try Y: Div and the voltage cursors.",
        file: "example-1013d-500khz-2ch.wav",
        bytes: include_bytes!("../assets/examples/1013d/15.wav"),
    },
    Example {
        label: "Slow signal, about 1 Hz",
        about: "1013D, 2 s/div: a 1 minute record.",
        file: "example-1013d-1hz-slow.wav",
        bytes: include_bytes!("../assets/examples/1013d/12.wav"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_examples_parse() {
        for e in EXAMPLES {
            let w = crate::parser::parse(e.bytes).unwrap_or_else(|err| panic!("{}: {err}", e.file));
            assert!(w.ch1.is_some(), "{}", e.file);
        }
    }
}
