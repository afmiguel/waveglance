# fnirsi-viewer

A viewer for the waveform files (`.wav`) that the FNIRSI DPOX180H handheld
oscilloscope saves. It runs on the desktop and in a web browser (WebAssembly).

The DPOX180H `.wav` file is **not** an audio file. It is a big-endian dump of
the scope state: settings, a small screen thumbnail and the raw ADC samples.

## Use

- Drop a `.wav` file on the window, or click **Open file…**.
- Mouse wheel: zoom. Shift + wheel: time axis only. Alt + wheel: voltage axis only.
- Drag: pan. Double-click: reset the view.
- **Fit H**: full time span. **Fit V**: voltage axis fitted to the samples between
  the first and last instant shown. **Fit both**: Fit H, then Fit V.
- **Cursors** (Off / Time / Volt / Both): drag a dashed cursor line to move it.
  Time cursors show T1, T2, ΔT, 1/ΔT and each channel's voltage at T1 and T2
  (linear interpolation). Voltage cursors show V1, V2 and ΔV.
- The time axis has the trigger at t = 0. The voltage axis is in volts at the
  probe tip (the V/div that the scope shows already includes the probe setting).

## Build and run

Desktop:

```bash
cargo run --release
```

Web (needs `rustup target add wasm32-unknown-unknown` and `cargo install trunk`):

```bash
trunk serve --release
```

Static build for GitHub Pages (the site is served from `/<repository>/`):

```bash
trunk build --release --public-url /fnirsi-viewer/
```

Then publish the `dist/` folder. The workflow `.github/workflows/pages.yml` does
this on each push to `main`.

## Tests

Unit tests use synthetic files. The integration tests in `tests/real_samples.rs`
use real DPOX180H files from
[Rav263/fnirsi-wave-decoder](https://github.com/Rav263/fnirsi-wave-decoder).
That repository has no license, so the files are not stored here. Download them
first:

```bash
./scripts/fetch-samples.sh
cargo test
```

## File format

All values are big-endian. Offsets are from the start of the file.

| Offset | Type | Field |
|--------|------|-------|
| 0x00 | 7 × u32 | Section table: settings start/size, screen start/size, waveform start/size, total size |
| 0x66 / 0xA6 | u8 | CH1 / CH2 enabled |
| 0x6E / 0xAE | u64 | CH1 / CH2 V/div in µV (as shown, includes the probe factor) |
| 0x7E / 0xBE | u16 | CH1 / CH2 ground marker, screen pixels (100 = centre, 25 px/div) |
| 0x82 / 0xC2 | u64 | CH1 / CH2 probe factor × 10000 (10000 = 1X, 100000 = 10X) |
| 0x12D | u32 | Samples per channel |
| 0x13D | u32 | Sample rate in Hz (overflows above 4.29 GS/s) |
| 0x1D9 | u32 | Time/div in ps |
| waveform start | u16[] | CH1 samples (if on), then CH2 samples (if on) |

Conversion:

- ADC codes: 0 to 12800, 1600 codes per division, centre 6400.
- Ground code = 6400 + (marker − 100) × 64.
- Voltage = (code − ground) / 1600 × V/div.
- Real-time mode: Δt = 1 / sample rate, trigger at the centre of the buffer.
- Equivalent-time (ETS) mode, at fast timebases: the buffer holds frames of
  300 points (one per screen pixel, 12 divisions). The viewer shows the first
  frame; Δt = 12 × time/div / 300, trigger at point 150.

The section table, enable flags, sample count and time/div come from the
firmware analysis in Rav263/fnirsi-wave-decoder. The V/div in µV, the probe
factor, the ground marker, the ETS frame layout and the trigger position were
found by comparing real files with the screen of the scope.

### Not verified yet

- A file with both channels on (the layout CH1-then-CH2 comes from the firmware
  analysis; it is tested only with synthetic files).
- The sample rate in real-time mode is verified only at 400 MS/s.
- A ground marker far from the centre, and slow (roll) timebases.
