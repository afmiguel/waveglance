//! Example files built into the app (File → Open example).
//!
//! - FNIRSI 1013D captures by lexus2026HA, CC BY-NC 4.0: see
//!   `assets/examples/1013d/README.md`.
//! - Rigol captures from the RigolWFM project by Scott Prahl, BSD 3-Clause:
//!   see `assets/examples/rigol/README.md`.
//! - VCD files made by `scripts/make-vcd-examples.py` for this project:
//!   see `assets/examples/vcd/README.md`.
//!
//! There are no DPOX180H examples yet: the only public DPOX180H files have
//! no license.

/// One built-in example file.
pub struct Example {
    /// Menu group: the name of a [`Group`].
    pub group: &'static str,
    /// Menu label.
    pub label: &'static str,
    /// What the example shows (menu tooltip).
    pub about: &'static str,
    /// File name shown after loading.
    pub file: &'static str,
    pub bytes: &'static [u8],
}

/// A group of examples from one source, with its credit (shown in the menu).
pub struct Group {
    pub name: &'static str,
    pub credit: &'static str,
}

pub const GROUPS: &[Group] = &[
    Group {
        name: "FNIRSI 1013D",
        credit: "Captures by lexus2026HA (github.com/lexus2026HA/FNIRSI-2C53T-Decoder), \
                 license CC BY-NC 4.0",
    },
    Group {
        name: "Rigol",
        credit: "From RigolWFM, copyright (c) 2020-23 Scott Prahl \
                 (github.com/scottprahl/RigolWFM), license BSD 3-Clause",
    },
    Group {
        name: "Logic (VCD)",
        credit: "Made for this viewer by scripts/make-vcd-examples.py (not captures)",
    },
];

pub const EXAMPLES: &[Example] = &[
    Example {
        group: "FNIRSI 1013D",
        label: "1 kHz square wave, CH1 and CH2",
        about: "1013D, 500 µs/div. CH1 1 V/div DC, about 3.2 Vpp. CH2 is on with no signal. \
                Try the measurements and the FFT (odd harmonics).",
        file: "example-1013d-1khz-square.wav",
        bytes: include_bytes!("../assets/examples/1013d/1.wav"),
    },
    Example {
        group: "FNIRSI 1013D",
        label: "1 kHz square wave, clipped",
        about: "1013D, 100 µs/div, CH1 0.5 V/div. The signal goes past the bottom of the \
                screen: the voltage measurements are marked with \"?\".",
        file: "example-1013d-clipped.wav",
        bytes: include_bytes!("../assets/examples/1013d/2.wav"),
    },
    Example {
        group: "FNIRSI 1013D",
        label: "Small AC signal, 50 mV/div",
        about: "1013D, 2 ms/div, AC coupling, about 36 mVpp: a low-level, noisy signal.",
        file: "example-1013d-small-ac.wav",
        bytes: include_bytes!("../assets/examples/1013d/3.wav"),
    },
    Example {
        group: "FNIRSI 1013D",
        label: "1 MHz signal",
        about: "1013D, 500 ns/div, CH1 1 V/div AC, about 5 Vpp.",
        file: "example-1013d-1mhz.wav",
        bytes: include_bytes!("../assets/examples/1013d/8.wav"),
    },
    Example {
        group: "FNIRSI 1013D",
        label: "10 MHz signal",
        about: "1013D, 100 ns/div, CH1 1 V/div AC: few samples per period.",
        file: "example-1013d-10mhz.wav",
        bytes: include_bytes!("../assets/examples/1013d/7.wav"),
    },
    Example {
        group: "FNIRSI 1013D",
        label: "500 kHz on two channels",
        about: "1013D, 1 µs/div. CH1 1 V/div and CH2 2.5 V/div, both AC. \
                Try Y: Div and the voltage cursors.",
        file: "example-1013d-500khz-2ch.wav",
        bytes: include_bytes!("../assets/examples/1013d/15.wav"),
    },
    Example {
        group: "FNIRSI 1013D",
        label: "Slow signal, about 1 Hz",
        about: "1013D, 2 s/div: a 1 minute record.",
        file: "example-1013d-1hz-slow.wav",
        bytes: include_bytes!("../assets/examples/1013d/12.wav"),
    },
    Example {
        group: "Rigol",
        label: "DHO1074: four channels (.bin)",
        about: "Rigol DHO1074 .bin file, 5 ms/div, four channels with different signals. \
                Use the channel buttons to show one at a time.",
        file: "example-rigol-dho1074.bin",
        bytes: include_bytes!("../assets/examples/rigol/DHO1074.bin"),
    },
    Example {
        group: "Rigol",
        label: "MSO5000: 1 kHz on four channels (.bin)",
        about: "Rigol MSO5000 .bin file, 500 µs/div. The 1 kHz calibration signal on \
                four channels: CH1 and CH4 about 3.2 Vpp, CH2 and CH3 about 1 Vpp.",
        file: "example-rigol-mso5000.bin",
        bytes: include_bytes!("../assets/examples/rigol/MSO5000-A.bin"),
    },
    Example {
        group: "Rigol",
        label: "DS2072A: 1 kHz, two channels (CSV)",
        about: "CSV file saved by a Rigol DS2072A: the 1 kHz calibration signal, \
                about 0.32 Vpp, on CH1 and CH2.",
        file: "example-rigol-ds2072a.csv",
        bytes: include_bytes!("../assets/examples/rigol/DS2072A-1.csv"),
    },
    Example {
        group: "Rigol",
        label: "DS1102E: 4 kHz, two channels (CSV)",
        about: "CSV file saved by a Rigol DS1102E: about 4.2 kHz and 6 Vpp on CH1 and CH2.",
        file: "example-rigol-ds1102e.csv",
        bytes: include_bytes!("../assets/examples/rigol/DS1102E-D.csv"),
    },
    Example {
        group: "Logic (VCD)",
        label: "CPU bus: write and read cycles",
        about: "A 10 MHz bus: address, data and strobes. The data bus is z (shown X) when no device drives it. Zoom in to read the values; right-click a bus readout at the bottom for Hex, Dec, Bin or ASCII.",
        file: "example-vcd-memory-bus.vcd",
        bytes: include_bytes!("../assets/examples/vcd/memory-bus.vcd"),
    },
    Example {
        group: "Logic (VCD)",
        label: "8-bit DAC: code and analog output",
        about: "A 500 Hz sine from an 8-bit DAC: the code bus, the load strobe and the filtered analog output in volts. Try the time cursors on the code and vout.",
        file: "example-vcd-dac-sine.vcd",
        bytes: include_bytes!("../assets/examples/vcd/dac-sine.vcd"),
    },
    Example {
        group: "Logic (VCD)",
        label: "UART: AT command and reply",
        about: "A host sends \"AT\" on tx and the device answers \"OK\" on rx, at 115200 baud \
                8N1. In Decode ▾, check tx and rx; Show ASCII to read the text.",
        file: "example-vcd-uart-at.vcd",
        bytes: include_bytes!("../assets/examples/vcd/uart-at.vcd"),
    },
    Example {
        group: "Logic (VCD)",
        label: "UART on an analog line (9600 baud)",
        about: "A 3.3 V serial line as an oscilloscope sees it: slow edges and noise. \
                Decode ▾ → UART finds the baud rate and the threshold; Show ASCII reads \
                \"Hello, FNIRSI!\".",
        file: "example-vcd-uart-analog.vcd",
        bytes: include_bytes!("../assets/examples/vcd/uart-analog.vcd"),
    },
    Example {
        group: "Logic (VCD)",
        label: "UART: complete session (4 lines)",
        about: "Four serial lines with different settings. host_tx and dev_tx: AT commands \
                and answers, 115200 8N1, then a break on dev_tx (framing error). gps: NMEA \
                sentences, 9600 8N1. rs232: an inverted line, 19200 7E1 (Data bits 7, \
                Parity Even, Idle Low), with one parity error. In Decode ▾, check the \
                lines and set rs232 in its ▸ settings; Show ASCII, and zoom in on the \
                fast lines.",
        file: "example-vcd-uart-session.vcd",
        bytes: include_bytes!("../assets/examples/vcd/uart-session.vcd"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_examples_parse() {
        for e in EXAMPLES {
            let w = crate::formats::open(e.bytes).unwrap_or_else(|err| panic!("{}: {err}", e.file));
            assert!(!w.traces.is_empty(), "{}", e.file);
            assert!(w.warnings.is_empty(), "{}: {:?}", e.file, w.warnings);
            assert!(GROUPS.iter().any(|g| g.name == e.group), "{}", e.file);
        }
    }

    fn decode_with(
        file: &str,
        label: &str,
        uart: crate::decode::uart::Settings,
    ) -> crate::decode::Decoded {
        let e = EXAMPLES.iter().find(|e| e.file.contains(file)).unwrap();
        let w = crate::formats::open(e.bytes).unwrap();
        let settings = crate::decode::Settings {
            protocol: crate::decode::Protocol::Uart,
            uart,
            ..Default::default()
        };
        let d = crate::decode::decode(w.trace(label).unwrap(), &settings).unwrap();
        assert!(d.warnings.is_empty(), "{:?}", d.warnings);
        d
    }

    fn text(d: &crate::decode::Decoded) -> String {
        d.frames
            .iter()
            .map(|f| char::from_u32(f.value).unwrap())
            .collect()
    }

    fn decode_text(file: &str, label: &str) -> String {
        let d = decode_with(file, label, Default::default());
        assert!(d.frames.iter().all(|f| f.error.is_none()));
        text(&d)
    }

    #[test]
    fn uart_session_decodes_with_its_settings() {
        use crate::decode::{FrameError, uart};
        let host = decode_text("uart-session", "host_tx");
        assert_eq!(host, "AT\r\nAT+TEMP?\r\nAT+NAME=FNIRSI\r\nAT+VER?\r\n");
        // dev_tx: the answers, then the break (a 0 with a framing error).
        let dev = decode_with("uart-session", "dev_tx", Default::default());
        let (last, answers) = dev.frames.split_last().unwrap();
        assert_eq!(last.error, Some(FrameError::Framing));
        assert_eq!(last.value, 0);
        assert!(answers.iter().all(|f| f.error.is_none()));
        assert!(text(&dev).starts_with("\r\nOK\r\n\r\n+TEMP: 23.5 C\r\n"));
        let gps = decode_text("uart-session", "gps");
        assert!(gps.starts_with("$GPRMC,123519,A,"));
        assert_eq!(gps.matches("\r\n").count(), 2);
        assert!(dev.title.starts_with("115200"));
        // rs232: 19200 7E1, idle low; character 9 has a wrong parity bit.
        let rs = decode_with(
            "uart-session",
            "rs232",
            uart::Settings {
                data_bits: 7,
                parity: uart::Parity::Even,
                idle_low: true,
                ..Default::default()
            },
        );
        assert_eq!(text(&rs), "ID?\rFNIRSI DPOX180H\r");
        let errors: Vec<usize> = rs
            .frames
            .iter()
            .enumerate()
            .filter(|(_, f)| f.error == Some(FrameError::Parity))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(errors, [9]);
        assert!(rs.title.starts_with("19200"));
    }

    #[test]
    fn uart_examples_decode() {
        assert_eq!(decode_text("uart-at", "tx"), "AT\r\n");
        assert_eq!(decode_text("uart-at", "rx"), "\r\nOK\r\n");
        assert_eq!(decode_text("uart-analog", "tx_pin"), "Hello, FNIRSI!\r\n");
    }

    #[test]
    fn memory_bus_reads_back_what_was_written() {
        let e = EXAMPLES
            .iter()
            .find(|e| e.file.contains("memory-bus"))
            .unwrap();
        let w = crate::formats::open(e.bytes).unwrap();
        let at = |label: &str, t: f64| {
            let tr = w.trace(label).unwrap_or_else(|| panic!("{label}"));
            crate::cursors::value_at(tr.points(), t).unwrap()
        };
        assert_eq!(w.trace("data[7:0]").and_then(|t| t.bus), Some(8));
        // Write cycle 2: 0x47 to 0x2002; read cycle 8 reads it back.
        assert_eq!(at("addr[15:0]", 3.7e-6), f64::from(0x2002));
        assert_eq!(at("data[7:0]", 3.7e-6), f64::from(0x47));
        assert_eq!(at("rd_n", 3.7e-6), 0.0);
        // Between cycles nobody drives the data bus: z, shown as X.
        assert!(at("data[7:0]", 3.45e-6).is_nan());
    }
}
