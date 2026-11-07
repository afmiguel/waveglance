# fnirsi-viewer

A viewer for the waveform files (`.wav`) that FNIRSI oscilloscopes save. It
runs on the desktop and in a web browser (WebAssembly).

Supported files (the format is found automatically from the file; if it is
not certain, a window asks):

| Model | File | Notes |
|-------|------|-------|
| DPOX180H | `.wav`, variable size, big-endian | Full ADC buffer; real-time and equivalent-time modes |
| 1013D, 1014D | `.wav`, 15000 bytes, little-endian | Original FNIRSI firmware only. 1500-sample buffer (30 divisions), coupling and trigger settings. The 1014D uses the 1013D format, but no 1014D file was tested |
| Keysight / Agilent | `.bin` (AG01, AG03, AG10) | Up to 4 channels; volts or amperes; no V/div in the file (a V/div that fits is used) |
| VCD | `.vcd` (logic analyzers, MSO, simulators) | 1-bit signals as digital lanes, `real` variables as analog traces; buses not shown yet |
| CSV / text, any oscilloscope | `.csv`, `.txt`: time + channel columns | Keysight, Rigol, Tektronix, Siglent layouts and this viewer's export; `,` `;` tab separators |

These `.wav` files are **not** audio files. Each model has its own binary
format, and FNIRSI does not publish them. Other FNIRSI models (2C53T, 2C53P,
2C23T and others) save other formats and are not supported.

## Use

- Drop a `.wav` file on the window, or use **⏏ File ▾ → Open file…** (Ctrl/⌘+O).
- **File ▾ → Open example**: real FNIRSI 1013D captures built into the app.
- **File ▾ → Open reference…**: a second file drawn faint behind the traces,
  to compare. The REF readout at the bottom hides or shows it.
- **File ▾ → Export CSV…**, **Save screenshot (PNG)…**, **Print…** (Ctrl/⌘+P,
  on a white screen).
- **◐ White / ◐ Black**: white screen with darker colours, for printing.
- Keyboard: Ctrl/⌘+O open, F1 help, F/H/V fit, Z zoom view, S FFT, Y volts/div,
  C cursors, Tab + arrows move a cursor, 1/2 hide a channel, +/− zoom, arrows pan.
- Mouse wheel on the CH1 / CH2 readout: V/div of that channel (1-2-5 steps,
  Div mode; * = not the file's V/div).
- Drag a ground marker (1▶, 2▶) to move that channel (this switches Y to Div,
  where each channel has its own scale).
- The welcome screen lists all the controls. The **? Help** button has the main instructions.
- Mouse wheel: zoom. Shift + wheel: time axis only. Alt + wheel: voltage axis only.
- Mouse wheel over a time or voltage axis: pan along that axis.
- Drag: pan. Double-click: reset the view.
- **⊕ Zoom**: overview of the whole record above the plot; the bright window
  is the part shown. Click or drag in it to move there, wheel on it to zoom.
- **Y ▾** menu: **Volts / Div**. *Volts* (default): one voltage axis for both channels.
  *Div* is like the scope screen: 8 divisions, each channel drawn with its own
  V/div from its ground marker (**1▶**, **2▶** on the left edge).
- Click the **CH1** or **CH2** readout at the bottom to hide or show that
  channel (its trace, ground marker, measurements and cursor values).
- **⛶ Fit ▾** menu. **Fit H**: full time span. **Fit V**: voltage axis fitted to the samples between
  the first and last instant shown. **Fit both**: Fit H, then Fit V.
- **⌖ Cursors** menu (Off / Time / Volt / Both, and *Bring to view*): drag a
  dashed cursor line to move it.
  Time cursors show T1, T2, ΔT, 1/ΔT and each channel's voltage at T1 and T2
  (linear interpolation). Voltage cursors show V1, V2 and ΔV.
- **Σ Math ▾**: CH1 + CH2, CH1 − CH2 or CH2 − CH1, measured like a channel.
- **⌕ Search ▾**: find rising edges, falling edges or glitches in the whole
  record; [ and ] go to the previous and next one.
- **▤ Measure → Statistics per cycle**: mean, min, max, σ and n of the time
  measurements, in a floating window.
- **∿ FFT ▾** menu: spectrum of CH1 or CH2 under the plot, of the samples
  shown in the time plot (zoom in to analyse a part). Windows Rectangular,
  Hanning, Hamming, Blackman-Harris and Flat top; scale dBV or V RMS; peak
  readout (frequency interpolated between bins), Δf and N.
- **▤ Measure**: choose the automatic measurements to show (Freq, Period, Vpp,
  Vmax, Vmin, Vtop, Vbase, Vamp, Mean, RMS, Rise, Fall, +Width, −Width, +Duty),
  the channels, and the samples to measure: **Screen** (the time span shown
  now), **Record** (the whole file) or **T1–T2** (between the time cursors).
  The choice is saved between sessions.
  - Vtop and Vbase are the most common high and low levels (histogram).
  - Edges use the 10%, 50% and 90% levels between Vbase and Vtop, with
    hysteresis. Glitches (two edges closer than 10% of the typical edge
    spacing) are ignored.
  - With an amplitude below 0.1 division, the signal is taken as noise and
    the time measurements show "—".
- Readouts use 4 significant digits, and never more resolution than the
  samples have (one ADC code for voltages; Rise/Fall shorter than 2 samples
  show "<…"). If the signal is clipped in the measured window, its voltage
  measurements are orange and start with "?": the real value can be larger.
- If a file cannot be opened, a banner says why; the file shown before stays.
- The time axis has the trigger at t = 0 (the "T" marker on the top edge). The voltage axis is in volts at the
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

## Code structure

| Path | Contents |
|------|----------|
| `src/model.rs` | Internal model: `Capture`, `Trace`, `Samples`, display settings |
| `src/formats/` | One module per file format, detection (`probe`) and `open` |
| `src/app/` | User interface: `mod.rs` (state), `menus`, `readouts`, `plot`, `input`, `dialogs`, `actions`, `format` |
| `src/measure.rs`, `src/fft.rs`, `src/cursors.rs` | Measurements, spectrum, cursors (work on any trace) |
| `src/export.rs`, `src/examples.rs`, `src/palette.rs` | CSV/PNG/print, built-in examples, colours |

To add a file format: write a module in `src/formats/` that implements
`Format` (`name`, `probe`, `read` → `Capture`), add it to `FORMATS`, and add
tests with real files.

## Versions

See [CHANGELOG.md](CHANGELOG.md). The version is in `Cargo.toml`. The app shows
it next to the title (hover it for the git commit), so each build can be
identified, also between releases. To release: raise the version in
`Cargo.toml`, add a CHANGELOG entry, commit, and tag `vX.Y.Z`.

## Tests

Unit tests use synthetic files. The integration tests in `tests/real_samples.rs`
use real files: DPOX180H files from
[Rav263/fnirsi-wave-decoder](https://github.com/Rav263/fnirsi-wave-decoder)
(no license) and 1013D files from lexus2026HA/FNIRSI-2C53T-Decoder
(CC BY-NC 4.0), and Keysight/Agilent `.bin` files from
[scottprahl/RigolWFM](https://github.com/scottprahl/RigolWFM) (BSD-3-Clause).
The files are not stored here. Download them
first:

```bash
./scripts/fetch-samples.sh
cargo test
```

## File format: DPOX180H

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

## File format: 1013D / 1014D

15000 bytes, little-endian. Bytes 0–999 are the header; CH2 settings are at the
CH1 offset + 10.

| Byte | Field |
|------|-------|
| 2 / 12 | CH1 / CH2 shown (0 = off) |
| 4 / 14 | V/div index: 5, 2.5, 1, 0.5, 0.2, 0.1, 0.05 V (times the probe) |
| 8 / 18 | Coupling: 0 = DC, 1 = AC |
| 10 / 20 | Probe: 10^n (1X, 10X, 100X) |
| 22 | Time/div index 0..29: 50 s … 10 ns (1-2-5 steps) |
| 26 / 28 / 30 | Trigger mode (Auto, Single, Normal) / edge (0 = rising) / source (0 = CH1) |
| 84 / 86 | u16: buffer value of 0 V for CH1 / CH2 |
| 1000 / 4000 | CH1 / CH2 buffer: 1500 × u16 |

- Voltage = (value − zero) / 50 × V/div. Δt = time/div / 50.
- The trigger is at the centre of the buffer (sample 750).
- Sources: Robert Tidey
  ([FNISR1013DScope](https://github.com/roberttidey/FNISR1013DScope)), W. Voigt
  (zero level) and
  [lexus2026HA/FNIRSI-2C53T-Decoder](https://github.com/lexus2026HA/FNIRSI-2C53T-Decoder)
  (header bytes checked against screenshots). The tests use 21 real files from
  that last repository (CC BY-NC 4.0, downloaded by `scripts/fetch-samples.sh`,
  not stored here).
- Other firmware (Peco, Atlan) writes another layout. Such files are rejected
  with an error message.

## Example files

The files in `assets/examples/1013d/` are FNIRSI 1013D captures by
lexus2026HA, from
[lexus2026HA/FNIRSI-2C53T-Decoder](https://github.com/lexus2026HA/FNIRSI-2C53T-Decoder),
licensed [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/). They
are not changed. There are no DPOX180H examples: the only public DPOX180H files
have no license.
