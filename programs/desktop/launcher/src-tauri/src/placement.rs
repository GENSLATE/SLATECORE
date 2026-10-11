//! Where the launcher window goes, as pure functions over physical pixels (no Tauri types).
//!
//! The window is fixed in the bottom-right corner of the work area of the monitor under the
//! cursor, sized with *that* monitor's scale factor. Nothing may move it: when a `Moved` event
//! says it left its place, [`snap_back`] gives the rectangle to put it back to, on whichever
//! monitor it ended up on. [`cursor_in_frame`] is the click-through test.

use genslate_launcher_core::geometry::{self, EDGE_MARGIN, Rect};

/// How far (physical px) the window may differ from its place before it is put back. Windows
/// rounds sizes and positions, so exact equality would make the shell and the OS argue.
const TOLERANCE: i64 = 1;

/// One monitor, as the shell reads it from Tauri.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorInfo {
    /// The whole monitor.
    pub screen: Rect,
    /// The monitor minus the taskbar.
    pub work: Rect,
    /// DPI scale factor (1.0 = 100 percent).
    pub scale: f64,
}

fn anchored(monitor: &MonitorInfo, logical: (f64, f64)) -> Rect {
    geometry::anchor_bottom_right(monitor.work, monitor.scale, logical, EDGE_MARGIN)
}

fn screens(monitors: &[MonitorInfo]) -> Vec<Rect> {
    monitors.iter().map(|monitor| monitor.screen).collect()
}

/// The rectangle for showing the window: the bottom-right corner of the work area of the
/// monitor under `cursor` (else the nearest one), `logical` px wide and high at that monitor's
/// scale factor. Returns the monitor's index and its scale factor with the rectangle.
pub fn target_rect(
    monitors: &[MonitorInfo],
    cursor: (f64, f64),
    logical: (f64, f64),
) -> Option<(Rect, f64)> {
    let index = geometry::pick_monitor(cursor, &screens(monitors))?;
    let monitor = monitors.get(index)?;
    Some((anchored(monitor, logical), monitor.scale))
}

/// Where a window that now sits at `actual` belongs, or `None` when it is already there.
///
/// The monitor is the one under the window's centre, so a window dragged (or thrown by
/// Win+Shift+Arrow) to another screen lands in that screen's corner.
pub fn snap_back(
    actual: Rect,
    monitors: &[MonitorInfo],
    logical: (f64, f64),
) -> Option<(Rect, f64)> {
    let centre = (
        f64::from(actual.x) + f64::from(actual.width) / 2.0,
        f64::from(actual.y) + f64::from(actual.height) / 2.0,
    );
    let (wanted, scale) = target_rect(monitors, centre, logical)?;
    let off = |a: i64, b: i64| (a - b).abs() > TOLERANCE;
    let moved = off(i64::from(actual.x), i64::from(wanted.x))
        || off(i64::from(actual.y), i64::from(wanted.y))
        || off(i64::from(actual.width), i64::from(wanted.width))
        || off(i64::from(actual.height), i64::from(wanted.height));
    moved.then_some((wanted, scale))
}

/// Whether the window, now `actual` (width, height) in physical pixels, has the size `wanted`.
///
/// A window moved to a monitor with another scale factor adopts that monitor's DPI and Windows
/// may rescale its size on the way, so a `set_size` made before the move settles can come out
/// wrong. The shell places the window, reads the size back and places it once more when this says
/// no, before the window is shown.
pub fn size_settled(actual: (u32, u32), wanted: Rect) -> bool {
    let close = |a: u32, b: u32| (i64::from(a) - i64::from(b)).abs() <= TOLERANCE;
    close(actual.0, wanted.width) && close(actual.1, wanted.height)
}

/// Whether `cursor` is over the visible frame of the window at `window` (the rest of the window
/// is transparent and lets clicks through).
pub fn cursor_in_frame(window: Rect, scale: f64, expanded: bool, cursor: (f64, f64)) -> bool {
    geometry::frame_rect(window, scale, expanded).contains(cursor.0, cursor.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use genslate_launcher_core::config::SizePreset;
    use std::error::Error;

    type TestResult = Result<(), Box<dyn Error>>;

    const fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// A 100 percent 1920x1080 primary with a 48 px taskbar, and a 200 percent 3840x2160
    /// monitor to its right with a 96 px taskbar.
    fn two_monitors() -> [MonitorInfo; 2] {
        [
            MonitorInfo {
                screen: rect(0, 0, 1920, 1080),
                work: rect(0, 0, 1920, 1032),
                scale: 1.0,
            },
            MonitorInfo {
                screen: rect(1920, 0, 3840, 2160),
                work: rect(1920, 0, 3840, 2064),
                scale: 2.0,
            },
        ]
    }

    fn logical() -> (f64, f64) {
        geometry::window_size(SizePreset::M)
    }

    fn place(monitors: &[MonitorInfo], cursor: (f64, f64)) -> Result<(Rect, f64), &'static str> {
        target_rect(monitors, cursor, logical()).ok_or("no monitor")
    }

    fn is(scale: f64, wanted: f64) -> bool {
        (scale - wanted).abs() < f64::EPSILON
    }

    #[test]
    fn placement_uses_target_monitor_scale_factor() -> TestResult {
        let monitors = two_monitors();
        let (primary, scale) = place(&monitors, (100.0, 100.0))?;
        assert!(is(scale, 1.0));
        assert_eq!((primary.width, primary.height), (952, 672));
        assert_eq!((primary.x, primary.y), (1920 - 952, 1032 - 672));

        let (second, scale) = place(&monitors, (4000.0, 500.0))?;
        assert!(is(scale, 2.0));
        assert_eq!(
            (second.width, second.height),
            (1904, 1344),
            "twice the logical size on the 200 percent monitor"
        );
        assert_eq!(
            (second.x, second.y),
            (1920 + 3840 - 1904, 2064 - 1344),
            "bottom-right of that monitor's work area"
        );
        Ok(())
    }

    #[test]
    fn a_cursor_off_every_monitor_picks_the_nearest_one() -> TestResult {
        let monitors = two_monitors();
        let (rect, scale) = place(&monitors, (9000.0, 9000.0))?;
        assert!(is(scale, 2.0));
        assert!(rect.x >= 1920);
        assert!(target_rect(&[], (0.0, 0.0), logical()).is_none());
        Ok(())
    }

    #[test]
    fn window_snaps_back_to_bottom_right_after_moved_event() -> TestResult {
        let monitors = two_monitors();
        let (home, _) = place(&monitors, (100.0, 100.0))?;
        assert_eq!(
            snap_back(home, &monitors, logical()),
            None,
            "already in place"
        );
        let within_rounding = rect(home.x + 1, home.y - 1, home.width, home.height);
        assert_eq!(snap_back(within_rounding, &monitors, logical()), None);

        // Dragged to the middle of the first monitor: back to its corner.
        let dragged = rect(400, 300, home.width, home.height);
        assert_eq!(snap_back(dragged, &monitors, logical()), Some((home, 1.0)));

        // Thrown onto the second monitor: that monitor's corner, at its scale.
        let thrown = rect(2500, 200, home.width, home.height);
        let (corner, scale) = snap_back(thrown, &monitors, logical()).ok_or("no snap")?;
        assert!(is(scale, 2.0));
        assert_eq!(
            (
                corner.x + i32::try_from(corner.width)?,
                corner.y + i32::try_from(corner.height)?
            ),
            (5760, 2064)
        );

        // A resize (a DPI change on arrival) is put right too.
        let resized = rect(home.x, home.y, home.width + 40, home.height);
        let fixed = snap_back(resized, &monitors, logical()).map(|(r, _)| r.width);
        assert_eq!(fixed, Some(home.width));
        Ok(())
    }

    #[test]
    fn a_new_height_preset_re_anchors_in_the_same_corner_on_the_same_monitor() -> TestResult {
        let monitors = two_monitors();
        // Shown on the 200 percent monitor at size M ...
        let (home, scale) = place(&monitors, (4000.0, 500.0))?;
        assert!(is(scale, 2.0));
        // ... then the preset becomes L: the window is moved and resized in place, on the same
        // monitor, to the bottom-right corner of its work area.
        let large = geometry::window_size(SizePreset::L);
        let (grown, grown_scale) = snap_back(home, &monitors, large).ok_or("did not re-anchor")?;
        assert!(is(grown_scale, 2.0), "still the second monitor");
        assert!(grown.height > home.height);
        assert_eq!(grown.width, home.width, "the width never changes");
        assert_eq!(
            (
                grown.x + i32::try_from(grown.width)?,
                grown.y + i32::try_from(grown.height)?
            ),
            (5760, 2064),
            "bottom-right of that monitor's work area"
        );
        // Placing it there again is a no-op: no loop of Moved events.
        assert_eq!(snap_back(grown, &monitors, large), None);
        Ok(())
    }

    #[test]
    fn a_size_that_did_not_settle_is_detected_within_one_pixel() -> TestResult {
        let monitors = two_monitors();
        let (target, _) = place(&monitors, (4000.0, 500.0))?;
        assert!(size_settled((target.width, target.height), target));
        assert!(size_settled((target.width + 1, target.height - 1), target));
        // Moved from a 100 percent monitor: the size is still the old monitor's pixels.
        assert!(!size_settled((952, 672), target));
        assert!(!size_settled((target.width, target.height + 2), target));
        Ok(())
    }

    #[test]
    fn the_transparent_margin_is_click_through() {
        // A 952 x 672 window whose right edge is at 1920: the frame ends 16 px short of it.
        let window = rect(968, 408, 952, 672);
        let inside = (1800.0, 700.0);
        let inset = (975.0, 700.0);
        let left_half = (1000.0, 700.0);
        assert!(cursor_in_frame(window, 1.0, false, inside));
        assert!(!cursor_in_frame(window, 1.0, false, inset));
        assert!(
            !cursor_in_frame(window, 1.0, false, left_half),
            "apps list: the left half of the window is transparent"
        );
        assert!(
            cursor_in_frame(window, 1.0, true, left_half),
            "tools open: the frame is wider"
        );
        assert!(!cursor_in_frame(window, 1.0, true, inset));
    }
}
