//! Example files built into the app (File → Open example).
//!
//! - DPOX180H captures saved for this project by its author: see
//!   `assets/examples/dpox180h/README.md`.
//! - FNIRSI 1013D captures by lexus2026HA, CC BY-NC 4.0: see
//!   `assets/examples/1013d/README.md`.
//! - Rigol captures from the RigolWFM project by Scott Prahl, BSD 3-Clause:
//!   see `assets/examples/rigol/README.md`.
//! - VCD files made by `scripts/make-vcd-examples.py` for this project:
//!   see `assets/examples/vcd/README.md`.

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

impl Example {
    /// The short name of the example, for a link: the file name without
    /// "example-" and the extension ("vcd-uart-session").
    pub fn id(&self) -> &'static str {
        let name = self.file.strip_prefix("example-").unwrap_or(self.file);
        name.rsplit_once('.').map_or(name, |(stem, _)| stem)
    }
}

/// The example with short name `id` (see [`Example::id`]).
pub fn find(id: &str) -> Option<&'static Example> {
    EXAMPLES.iter().find(|e| e.id() == id)
}

/// The address of the site.
pub const SITE: &str = "https://afmiguel.github.io/waveglance/";

/// A link that opens example `e` on the site.
pub fn link(e: &Example) -> String {
    format!("{SITE}?example={}", e.id())
}

/// A group of examples from one source, with its credit (shown in the menu).
pub struct Group {
    pub name: &'static str,
    pub credit: &'static str,
}

pub const GROUPS: &[Group] = &[
    Group {
        name: "FNIRSI DPOX180H",
        credit: "Captures saved on a DPOX180H by the author of this viewer",
    },
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
        group: "FNIRSI DPOX180H",
        label: "1 kHz square wave, CH1",
        about: "DPOX180H, 500 µs/div, CH1 1 V/div DC: a 1 kHz square wave of about 1.6 Vpp. \
                48000 samples at 4 MS/s; trigger Auto, rising edge on CH1.",
        file: "example-dpox180h-1khz-square.wav",
        bytes: include_bytes!("../assets/examples/dpox180h/square-1khz.wav"),
    },
    Example {
        group: "FNIRSI DPOX180H",
        label: "Two channels, CH2 in AC",
        about: "DPOX180H, 500 µs/div. CH1 1 V/div DC and CH2 2 V/div AC coupling, \
                the same 1 kHz square wave (small on CH2). Try Y: Div and the channel \
                readouts at the bottom.",
        file: "example-dpox180h-two-channels.wav",
        bytes: include_bytes!("../assets/examples/dpox180h/two-channels.wav"),
    },
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
    Example {
        group: "Logic (VCD)",
        label: "I2C: temperature sensor",
        about: "I2C at 100 kHz: write a register of a sensor at 0x48, read the \
                temperature with a repeated start (0x19 0x80 = 25.5 °C, the last byte \
                with a NACK), and an address that nobody answers (0x50, NACK). In \
                Decode ▾, check sda: it uses I2C with scl as clock.",
        file: "example-vcd-i2c-sensor.vcd",
        bytes: include_bytes!("../assets/examples/vcd/i2c-sensor.vcd"),
    },
    Example {
        group: "Logic (VCD)",
        label: "I2C on two analog channels",
        about: "The temperature read as an oscilloscope sees it: CH1 is SDA, CH2 is \
                SCL, 3.3 V open-drain lines with slow rising edges and noise. In \
                Decode ▾, check CH1, then in its ▸ settings choose I2C and CH2 as SCL.",
        file: "example-vcd-i2c-analog.vcd",
        bytes: include_bytes!("../assets/examples/vcd/i2c-analog.vcd"),
    },
    Example {
        group: "Logic (VCD)",
        label: "SPI: flash memory",
        about: "SPI at 1 MHz, mode 0, with cs, sck, mosi and miso: read the JEDEC ID \
                (0x9F, answer EF 40 18), write enable (0x06), and read 9 bytes \
                (0x03 at 0x000100): \"Hello SPI\" on miso. In Decode ▾, check mosi \
                and miso: they use SPI with sck and cs by themselves.",
        file: "example-vcd-spi-flash.vcd",
        bytes: include_bytes!("../assets/examples/vcd/spi-flash.vcd"),
    },
    Example {
        group: "Logic (VCD)",
        label: "SPI on two analog channels (no CS)",
        about: "SPI as a two-channel oscilloscope sees it: CH1 is MOSI, CH2 is SCLK, \
                500 kHz, mode 0, no chip select. In Decode ▾, check CH1, then in its \
                ▸ settings choose SPI and CH2 as SCLK: the words are found from the \
                pauses of the clock.",
        file: "example-vcd-spi-analog.vcd",
        bytes: include_bytes!("../assets/examples/vcd/spi-analog.vcd"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_ids_are_unique_and_found() {
        let mut ids: Vec<&str> = EXAMPLES.iter().map(Example::id).collect();
        assert_eq!(
            find("vcd-uart-session").map(|e| e.file),
            Some("example-vcd-uart-session.vcd")
        );
        ids.sort_unstable();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n);
        assert!(find("nothing").is_none());
    }

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
        let d = crate::decode::decode(w.trace(label).unwrap(), None, None, &settings).unwrap();
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

    /// The I2C frames of an example, in short: "S W48 00 Sr R48 19 80N P".
    fn i2c_text(file: &str, sda: &str, scl: &str) -> String {
        use crate::decode::FrameKind;
        let e = EXAMPLES.iter().find(|e| e.file.contains(file)).unwrap();
        let w = crate::formats::open(e.bytes).unwrap();
        let settings = crate::decode::Settings {
            protocol: crate::decode::Protocol::I2c,
            ..Default::default()
        };
        let d =
            crate::decode::decode(w.trace(sda).unwrap(), w.trace(scl), None, &settings).unwrap();
        assert!(d.warnings.is_empty(), "{:?}", d.warnings);
        assert!(d.frames.iter().all(|f| f.error.is_none()));
        assert!(d.title.contains("100.0 kHz"), "{}", d.title);
        d.frames
            .iter()
            .map(|f| {
                let n = if f.ack == Some(false) { "N" } else { "" };
                match f.kind {
                    FrameKind::Start { repeated } => if repeated { "Sr" } else { "S" }.to_owned(),
                    FrameKind::Stop => "P".to_owned(),
                    FrameKind::Select { .. } => "CS".to_owned(),
                    FrameKind::Address { read } => {
                        format!("{}{:02X}{n}", if read { "R" } else { "W" }, f.value)
                    }
                    FrameKind::Data => format!("{:02X}{n}", f.value),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The SPI data words of an example, in hex.
    fn spi_words(file: &str, data: &str, clock: &str, cs: Option<&str>) -> Vec<u32> {
        use crate::decode::{FrameKind, Protocol, Settings};
        let e = EXAMPLES.iter().find(|e| e.file.contains(file)).unwrap();
        let w = crate::formats::open(e.bytes).unwrap();
        let settings = Settings {
            protocol: Protocol::Spi,
            ..Default::default()
        };
        let d = crate::decode::decode(
            w.trace(data).unwrap(),
            w.trace(clock),
            cs.and_then(|c| w.trace(c)),
            &settings,
        )
        .unwrap();
        assert!(d.warnings.is_empty(), "{:?}", d.warnings);
        assert!(d.frames.iter().all(|f| f.error.is_none()), "{:?}", d.frames);
        d.frames
            .iter()
            .filter(|f| f.kind == FrameKind::Data)
            .map(|f| f.value)
            .collect()
    }

    #[test]
    fn spi_examples_decode() {
        let mosi = spi_words("spi-flash", "mosi", "sck", Some("cs"));
        assert_eq!(mosi[..6], [0x9F, 0, 0, 0, 0x06, 0x03]);
        let miso = spi_words("spi-flash", "miso", "sck", Some("cs"));
        assert_eq!(miso[1..4], [0xEF, 0x40, 0x18]);
        let text: String = miso[9..]
            .iter()
            .map(|&v| char::from_u32(v).unwrap())
            .collect();
        assert_eq!(text, "Hello SPI");
        assert_eq!(
            spi_words("spi-analog", "CH1", "CH2", None),
            [0x9F, 0x03, 0x00, 0x01, 0x00]
        );
    }

    #[test]
    fn i2c_examples_decode() {
        assert_eq!(
            i2c_text("i2c-sensor", "sda", "scl"),
            "S W48 01 60 P S W48 00 Sr R48 19 80N P S W50N P"
        );
        assert_eq!(
            i2c_text("i2c-analog", "CH1", "CH2"),
            "S W48 00 Sr R48 19 80N P"
        );
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
    fn dpox180h_examples() {
        use crate::model::Coupling;
        let open = |part: &str| {
            let e = EXAMPLES.iter().find(|e| e.file.contains(part)).unwrap();
            crate::formats::open(e.bytes).unwrap()
        };
        let w = open("dpox180h-1khz");
        assert_eq!(w.source.model, "DPOX180H");
        assert_eq!(w.traces.len(), 1);
        assert_eq!(w.record_length, 48_000);
        let w = open("dpox180h-two");
        assert_eq!(w.traces.len(), 2);
        assert_eq!(w.trace("CH2").unwrap().display.coupling, Some(Coupling::Ac));
        assert_eq!(w.trace("CH1").unwrap().display.coupling, Some(Coupling::Dc));
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
