# Changelog

Versions follow [Semantic Versioning](https://semver.org/): a new feature
raises the minor number (0.2.0 → 0.3.0), a fix raises the patch number
(0.2.0 → 0.2.1). Each release has a git tag `vX.Y.Z`. The app shows its
version next to the title; hover it to see the git commit of the build.

## 0.11.1 — 2026-11-07

- The Open file dialog lists .bin and .vcd files too (before, they were
  only under "All files").

## 0.11.0 — 2026-11-07

- **Keysight / Agilent `.bin`** files (InfiniiVision and others, versions
  AG01/AG03/AG10): up to 4 channels, current channels in amperes, model from
  the file, time/div from the display range. Tested with real files: the
  time/div of 4 files and the scope's own frequency and Vpp measurements
  match; a 4-channel MSO-X 4154A file with 2 million points per channel.
- **VCD** (logic analyzers, mixed-signal scopes, simulators): 1-bit signals
  as digital traces (steps, in lanes under the analog traces, with their
  names; `x`/`z` drawn half-way) and `real` variables as analog traces.
  Buses (more than 1 bit) are not shown yet.
- Digital traces: no V/div ("logic" in the OSD), tooltip shows 0/1, voltage
  measurements are hidden; frequency, period, widths and duty work.
- Readouts use the unit of the trace (A for current channels).
- `scripts/fetch-samples.sh` also downloads the Keysight test files (from
  RigolWFM, BSD-3-Clause).

## 0.10.0 — 2026-11-07

- **CSV files from any oscilloscope** (and other delimited text): Keysight
  (names + units lines), Rigol (sample number with Start/Increment),
  Tektronix (settings in the first columns), Siglent, and this viewer's own
  export (which also gives back V/div, probe and coupling). Separators `,`
  `;` or tab; decimal comma with `;`; current channels (A). The maker is
  shown when the header names it.
- **Format detection with a choice window**: a file is read directly when
  one format is certain; otherwise "Which format is this file?" lists the
  possible formats and all the others. If a file cannot be read, the error
  banner has an **Open as…** button.
- **Large records**: the plot draws only the part in view, about two points
  per pixel; the Math channel and the measurements are cached. A 1-million-
  sample CSV (2 channels) zooms at 60 frames per second in the browser.
- File dialog: .wav, .csv, .txt and all files.

## 0.9.0 — 2026-11-07

Internal restructuring, to make room for other file formats. The app works
as before.

- New internal model (`src/model.rs`): a `Capture` with any number of
  `Trace`s; samples in volts (4 bytes each, evenly spaced, or time/value
  pairs); display settings (V/div, ground position, probe, coupling); input
  limits for clipping. No FNIRSI-specific fields any more.
- File formats in `src/formats/`: each format says how sure it is that a file
  is its own (`probe`) and reads it (`read`); `formats::open` tries them from
  the most to the least sure. FNIRSI DPOX180H and 1013D/1014D are the first two.
- The app handles N channels (colours for CH1–CH4) instead of exactly CH1 and
  CH2: OSD, measurements, cursors, FFT, search, reference, CSV export.
- Math: choose A and B among the channels (A + B or A − B).
- `app.rs` (3400 lines) split into modules: menus, readouts, plot, input,
  dialogs, actions and number formatting.

## 0.8.0 — 2026-11-07

- V/div of each channel: mouse wheel on the CH1 or CH2 readout (bottom bar)
  changes its V/div in 1-2-5 steps, like the V/div knob (switches Y to Div).
  A * marks a V/div that is not the one in the file; the measurements do not
  change. Y ▾ → Reset channel positions and scales puts everything back.

## 0.7.0 — 2026-11-07

- **Σ Math ▾**: CH1 + CH2, CH1 − CH2 or CH2 − CH1, sample by sample. The
  MATH trace is measured like a channel; click the MATH readout to hide it.
- **⌕ Search ▾**: rising edges, falling edges or glitches in the whole record
  of CH1 or CH2; marks on the bottom edge, Previous/Next (keys [ and ])
  centre the view on each event.
- Measure → **Statistics per cycle**: a floating window with mean, min, max,
  σ and number of cycles of the time measurements.
- **Print window**: white background (recommended, with the reason) or black;
  the screen changes only for the picture and comes back by itself.
- Toolbar: each button wraps to the next row as a whole (no button is cut
  off at the right edge); menu text wraps normally.

## 0.6.0 — 2026-11-07

- **⏏ File ▾** menu (was the Open file button): Open file…, Open example,
  Open reference…, Export CSV…, Save screenshot (PNG)…, Print….
- Open example: 7 real FNIRSI 1013D captures built into the app (by
  lexus2026HA, CC BY-NC 4.0; see `assets/examples/1013d/README.md`).
- Reference waveform: a second file drawn faint behind the traces, inside
  the time span of the main file; the REF readout hides or shows it.
- Export CSV: all samples (time and each channel's voltage) with the
  settings in `#` lines. Desktop: save dialog; browser: download.
- Save screenshot (PNG) and Print (Ctrl/⌘+P). Print uses the white screen;
  in the browser it opens the print dialog with a picture of the screen
  (eframe takes Ctrl/⌘+P from the browser, so the app handles it); on the
  desktop the picture opens in the image viewer.
- Favicon (SVG, with a PNG fallback).
- Fix: the Help icon is a plain "?", visible on the white screen too.
- Ground markers: a drag on 1▶ / 2▶ in Volts mode now switches to Div and
  moves that channel (in Volts both channels share one axis, so before the
  drag moved the whole plot). A tip on the marker explains it.

## 0.5.0 — 2026-11-07

- White screen (◐ White / ◐ Black, key B) for printing, with darker trace,
  cursor and marker colours (all at least 3:1 contrast on white); saved.
- Keyboard shortcuts: Ctrl/⌘+O open, F1 help, F/H/V fit, Z zoom view,
  S FFT, Y volts/div, C cursor mode, 1/2 hide/show a channel, +/− zoom,
  arrows move the view.
- Cursors: Tab selects the next cursor (thicker line), the arrows move it by
  one sample or 1/100 of the screen (Shift: 1/10). A click on a cursor
  selects it too.
- Div mode: drag the 1▶ / 2▶ ground marker to move a channel up or down;
  Y ▾ → Reset channel positions.
- Fix: scrolling the Help window (or a click on a menu) no longer zooms or
  moves the plot under it.
- Monochrome icons (⏏ Open file, ⊕ Zoom, ▤ Measure, ❔ Help), like the others.

## 0.4.0 — 2026-11-07

- FFT (∿ FFT menu): the spectrum of CH1 or CH2 under the time plot, of the
  samples shown in the time plot. Windows: Rectangular, Hanning, Hamming,
  Blackman-Harris, Flat top. Scale: dBV or V RMS (a sine of amplitude A reads
  A/√2). Peak readout with the frequency interpolated between bins, Δf and
  the number of samples. The settings are saved between sessions.

## 0.3.0 — 2026-11-07

- Zoom view (🔍 Zoom button): an overview of the whole record above the
  plot, with the part shown below as a bright window. Click or drag in the
  overview to move the view there; wheel on it to zoom in time. The overview
  draws min/max pairs, so peaks stay visible in long records. The switch is
  saved between sessions.

## 0.2.3 — 2026-11-07

- Fit and Y are menu buttons with ▾, like Cursors and Measure (Y uses radio
  buttons). The top bar is the same on wide and narrow screens.

## 0.2.2 — 2026-11-07

- The Measure button has the ▾ arrow, like the other menus.

## 0.2.1 — 2026-11-07

- A hidden channel's readout is only grey; the "(hidden)" text is gone.

## 0.2.0 — 2026-11-07

- First numbered release. All earlier builds said 0.1.0 (the version was not
  raised on each deploy); they are listed below.
- Y axis: Volts is the default again (Div is one click away).
- Help button with the main instructions.
- Cursors menu with radio buttons and *Bring to view*.
- Version label next to the title, with the git commit on hover.
- Welcome screen at startup (version, supported models, mouse/keyboard/touch
  controls), with "Don't show this at startup"; the **Help** button has the full instructions.
  The control hints moved there from the bottom bar.

## 0.1.0 — 2026-11-06 to 2026-11-07 (unreleased builds)

- DPOX180H `.wav` files; desktop and web (GitHub Pages).
- FNIRSI 1013D/1014D files (original firmware), with automatic model
  detection. The OSD shows the coupling and the trigger when the file has them.
- Y axis in divisions with per-channel ground markers, or in volts.
  Click the CH1/CH2 readout to hide or show a channel.
- Zoom, pan, mouse wheel on the axes, Fit H / Fit V / Fit both.
- Time and voltage cursors.
- Automatic measurements (15, selectable, saved between sessions) on the
  screen, the whole record or between the time cursors.
- Readouts with 4 significant digits, limited to the resolution of the
  samples; clipped measurements are marked.
- Trigger marker on the top edge; error banner when a file cannot be opened;
  layout for narrow screens.
