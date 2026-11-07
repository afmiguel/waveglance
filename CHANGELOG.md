# Changelog

Versions follow [Semantic Versioning](https://semver.org/): a new feature
raises the minor number (0.2.0 → 0.3.0), a fix raises the patch number
(0.2.0 → 0.2.1). Each release has a git tag `vX.Y.Z`. The app shows its
version next to the title; hover it to see the git commit of the build.

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
