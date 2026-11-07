//! Measurement cursors, like the cursors of a bench oscilloscope.
//!
//! - Time cursors T1 and T2 are vertical lines. Readout: T1, T2, ΔT, 1/ΔT and
//!   the channel voltages at T1 and T2 (linear interpolation).
//! - Voltage cursors V1 and V2 are horizontal lines. Readout: V1, V2, ΔV.
//!
//! Drag a cursor line to move it. Near a line, the drag moves the cursor and
//! the plot does not pan.

use eframe::egui::{self, Color32, CursorIcon, Pos2};
use egui_plot::{HLine, LineStyle, PlotBounds, PlotTransform, PlotUi, VLine};
use std::ops::RangeInclusive;

/// Colour of the time cursors (vertical lines).
pub const TIME_COLOR: Color32 = Color32::from_rgb(255, 110, 255);
/// Colour of the voltage cursors (horizontal lines).
pub const VOLT_COLOR: Color32 = Color32::from_rgb(120, 255, 120);
/// Distance in screen points at which the pointer grabs a cursor line.
pub const GRAB_DISTANCE: f32 = 6.0;

/// Which cursors are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorMode {
    #[default]
    Off,
    Time,
    Volt,
    Both,
}

impl CursorMode {
    pub fn shows_time(self) -> bool {
        matches!(self, Self::Time | Self::Both)
    }

    pub fn shows_volt(self) -> bool {
        matches!(self, Self::Volt | Self::Both)
    }
}

/// One cursor line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorId {
    T1,
    T2,
    V1,
    V2,
}

/// Cursor state.
#[derive(Debug, Clone, PartialEq)]
pub struct Cursors {
    pub mode: CursorMode,
    /// T1 and T2 in seconds.
    pub t: [f64; 2],
    /// V1 and V2 in volts.
    pub v: [f64; 2],
    /// The cursor that the pointer is moving now.
    pub dragging: Option<CursorId>,
    /// Time cursors must be placed in the visible area on the next frame.
    place_time: bool,
    /// Voltage cursors must be placed in the visible area on the next frame.
    place_volt: bool,
}

impl Default for Cursors {
    fn default() -> Self {
        Self {
            mode: CursorMode::Off,
            t: [0.0; 2],
            v: [0.0; 2],
            dragging: None,
            place_time: false,
            place_volt: false,
        }
    }
}

/// The value at 1/4 and 3/4 of a range.
fn quarters(range: &RangeInclusive<f64>) -> [f64; 2] {
    let (a, b) = (*range.start(), *range.end());
    [a + (b - a) * 0.25, a + (b - a) * 0.75]
}

impl Cursors {
    /// Change the mode. Cursors that become visible are placed in the
    /// visible area on the next frame.
    pub fn set_mode(&mut self, mode: CursorMode) {
        self.place_time |= mode.shows_time() && !self.mode.shows_time();
        self.place_volt |= mode.shows_volt() && !self.mode.shows_volt();
        self.mode = mode;
        self.dragging = None;
    }

    /// Put the cursors at 1/4 and 3/4 of the given ranges (used for a new file).
    pub fn place_in(&mut self, time: &RangeInclusive<f64>, volts: &RangeInclusive<f64>) {
        self.t = quarters(time);
        self.v = quarters(volts);
        self.place_time = false;
        self.place_volt = false;
        self.dragging = None;
    }

    /// Place the cursors that are waiting for a position in the visible area.
    pub fn place_pending(&mut self, visible: &PlotBounds) {
        if self.place_time {
            self.t = quarters(&visible.range_x());
            self.place_time = false;
        }
        if self.place_volt {
            self.v = quarters(&visible.range_y());
            self.place_volt = false;
        }
    }

    /// The visible cursor line nearest to `pos`, within `GRAB_DISTANCE`.
    pub fn hit(&self, transform: &PlotTransform, pos: Pos2) -> Option<CursorId> {
        if !transform.frame().contains(pos) {
            return None;
        }
        let mut lines = Vec::new();
        if self.mode.shows_time() {
            lines.push((
                CursorId::T1,
                (transform.position_from_point_x(self.t[0]) - pos.x).abs(),
            ));
            lines.push((
                CursorId::T2,
                (transform.position_from_point_x(self.t[1]) - pos.x).abs(),
            ));
        }
        if self.mode.shows_volt() {
            lines.push((
                CursorId::V1,
                (transform.position_from_point_y(self.v[0]) - pos.y).abs(),
            ));
            lines.push((
                CursorId::V2,
                (transform.position_from_point_y(self.v[1]) - pos.y).abs(),
            ));
        }
        lines
            .into_iter()
            .filter(|(_, d)| *d <= GRAB_DISTANCE)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    /// Start and stop drags.
    /// Call this before the plot is built. Returns `true` if the plot may pan
    /// (no cursor is being dragged).
    pub fn handle_pointer(
        &mut self,
        ctx: &egui::Context,
        last_transform: Option<&PlotTransform>,
    ) -> bool {
        if self.mode == CursorMode::Off {
            self.dragging = None;
            return true;
        }
        let (pos, pressed, down) = ctx.input(|i| {
            (
                i.pointer.latest_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
            )
        });
        if !down {
            self.dragging = None;
        }
        let near = match (pos, last_transform) {
            (Some(pos), Some(t)) => self.hit(t, pos),
            _ => None,
        };
        if pressed && self.dragging.is_none() {
            self.dragging = near;
        }
        self.dragging.is_none()
    }

    /// Show a resize pointer over a cursor line or while dragging one.
    /// Call this AFTER the plot is shown: the plot sets its own pointer
    /// (a crosshair), and the last call in a frame wins.
    pub fn update_pointer_icon(&self, ctx: &egui::Context, transform: &PlotTransform) {
        if self.mode == CursorMode::Off {
            return;
        }
        let near = ctx
            .input(|i| i.pointer.latest_pos())
            .and_then(|pos| self.hit(transform, pos));
        match self.dragging.or(near) {
            Some(CursorId::T1 | CursorId::T2) => ctx.set_cursor_icon(CursorIcon::ResizeHorizontal),
            Some(CursorId::V1 | CursorId::V2) => ctx.set_cursor_icon(CursorIcon::ResizeVertical),
            None => {}
        }
    }

    /// Move the dragged cursor to the pointer. Call this inside the plot.
    pub fn update_drag(&mut self, plot: &PlotUi<'_>) {
        let Some(id) = self.dragging else { return };
        let Some(pos) = plot.ctx().input(|i| i.pointer.latest_pos()) else {
            return;
        };
        let value = plot.transform().value_from_position(pos);
        match id {
            CursorId::T1 => self.t[0] = value.x,
            CursorId::T2 => self.t[1] = value.x,
            CursorId::V1 => self.v[0] = value.y,
            CursorId::V2 => self.v[1] = value.y,
        }
    }

    /// Draw the visible cursor lines.
    pub fn draw(&self, plot: &mut PlotUi<'_>) {
        let style = LineStyle::Dashed { length: 8.0 };
        let width = |id| if self.dragging == Some(id) { 2.0 } else { 1.2 };
        if self.mode.shows_time() {
            for (i, id) in [CursorId::T1, CursorId::T2].into_iter().enumerate() {
                plot.vline(
                    VLine::new(format!("T{}", i + 1), self.t[i])
                        .color(TIME_COLOR)
                        .style(style)
                        .width(width(id)),
                );
            }
        }
        if self.mode.shows_volt() {
            for (i, id) in [CursorId::V1, CursorId::V2].into_iter().enumerate() {
                plot.hline(
                    HLine::new(format!("V{}", i + 1), self.v[i])
                        .color(VOLT_COLOR)
                        .style(style)
                        .width(width(id)),
                );
            }
        }
    }

    /// ΔT = T2 − T1, in seconds.
    pub fn delta_t(&self) -> f64 {
        self.t[1] - self.t[0]
    }

    /// ΔV = V2 − V1, in volts.
    pub fn delta_v(&self) -> f64 {
        self.v[1] - self.v[0]
    }

    /// 1/|ΔT| in hertz, or `None` if ΔT is zero.
    pub fn frequency(&self) -> Option<f64> {
        let dt = self.delta_t().abs();
        (dt > 0.0).then(|| 1.0 / dt)
    }
}

/// Voltage of a waveform at time `t`, by linear interpolation between the
/// two nearest samples. `None` if `t` is outside the waveform.
/// `points` must be sorted by time.
pub fn value_at(points: &[[f64; 2]], t: f64) -> Option<f64> {
    let first = points.first()?;
    let last = points.last()?;
    if t < first[0] || t > last[0] {
        return None;
    }
    let i = points.partition_point(|p| p[0] < t);
    if i == 0 {
        return Some(first[1]);
    }
    let (a, b) = (points[i - 1], points[i]);
    if b[0] == a[0] {
        return Some(b[1]);
    }
    Some(a[1] + (b[1] - a[1]) * (t - a[0]) / (b[0] - a[0]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation() {
        let pts = [[0.0, 0.0], [1.0, 10.0], [2.0, 0.0]];
        assert_eq!(value_at(&pts, 0.0), Some(0.0));
        assert_eq!(value_at(&pts, 0.5), Some(5.0));
        assert_eq!(value_at(&pts, 1.0), Some(10.0));
        assert_eq!(value_at(&pts, 1.25), Some(7.5));
        assert_eq!(value_at(&pts, 2.0), Some(0.0));
        assert_eq!(value_at(&pts, -0.1), None);
        assert_eq!(value_at(&pts, 2.1), None);
        assert_eq!(value_at(&[], 0.0), None);
    }

    #[test]
    fn deltas_and_frequency() {
        let mut c = Cursors::default();
        c.place_in(&(0.0..=40e-6), &(-1.0..=1.0));
        assert!(
            (c.t[0] - 10e-6).abs() < 1e-18 && (c.t[1] - 30e-6).abs() < 1e-18,
            "{:?}",
            c.t
        );
        assert_eq!(c.v, [-0.5, 0.5]);
        assert!((c.delta_t() - 20e-6).abs() < 1e-18);
        assert!((c.frequency().unwrap() - 50e3).abs() < 1e-6);
        assert_eq!(c.delta_v(), 1.0);
        c.t = [1.0, 1.0];
        assert_eq!(c.frequency(), None);
    }

    #[test]
    fn mode_change_places_only_new_cursors() {
        let mut c = Cursors::default();
        c.place_in(&(0.0..=4.0), &(0.0..=4.0));
        c.set_mode(CursorMode::Time);
        let visible = PlotBounds::from_min_max([100.0, 100.0], [104.0, 104.0]);
        c.place_pending(&visible);
        assert_eq!(c.t, [101.0, 103.0]);
        assert_eq!(
            c.v,
            [1.0, 3.0],
            "voltage cursors are not shown, so they stay"
        );
        c.t = [102.0, 102.5];
        c.set_mode(CursorMode::Both);
        c.place_pending(&visible);
        assert_eq!(
            c.t,
            [102.0, 102.5],
            "time cursors were already shown, so they stay"
        );
        assert_eq!(c.v, [101.0, 103.0]);
    }
}
