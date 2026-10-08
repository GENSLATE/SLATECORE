//! Click debouncing for the tray, free of Tauri types so it is tested anywhere.
//!
//! Windows delivers a tray click as down, up (and a double click as down, up, double-click, up),
//! and the click moves focus to the taskbar, which hides the launcher on blur before the "up"
//! arrives. Two small guards deal with it: [`Debounce`] drops clicks that follow another
//! accepted click too closely, and [`RecentHide`] tells the toggle that the launcher was hidden
//! a moment ago, by the very click that is now asking to show it.

use std::time::{Duration, Instant};

/// How long after a click, or a hide, further tray clicks count as the same gesture.
pub const CLICK_WINDOW: Duration = Duration::from_millis(300);

/// Accepts an event only when `window` has passed since the last accepted one.
#[derive(Debug, Clone)]
pub struct Debounce {
    window: Duration,
    last: Option<Instant>,
}

impl Debounce {
    /// A debounce that ignores events within `window` of the last accepted one.
    pub const fn new(window: Duration) -> Self {
        Self { window, last: None }
    }

    /// `true` when the event at `now` is a new gesture (and is remembered as such).
    pub fn accept(&mut self, now: Instant) -> bool {
        if let Some(last) = self.last
            && now.saturating_duration_since(last) < self.window
        {
            return false;
        }
        self.last = Some(now);
        true
    }
}

/// Remembers when the launcher last hid.
#[derive(Debug, Clone, Default)]
pub struct RecentHide {
    hidden_at: Option<Instant>,
}

impl RecentHide {
    /// Records a hide at `now`.
    pub fn mark(&mut self, now: Instant) {
        self.hidden_at = Some(now);
    }

    /// `true` while a toggle at `now` is still the second half of the click that hid the window.
    pub fn blocks_toggle(&self, now: Instant) -> bool {
        self.hidden_at
            .is_some_and(|at| now.saturating_duration_since(at) < CLICK_WINDOW)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_debounce_ignores_clicks_within_300ms() {
        let start = Instant::now();
        let mut debounce = Debounce::new(Duration::from_millis(300));
        assert!(debounce.accept(start), "the first click counts");
        assert!(!debounce.accept(start + Duration::from_millis(1)));
        assert!(!debounce.accept(start + Duration::from_millis(299)));
        assert!(debounce.accept(start + Duration::from_millis(300)));
        // The window restarts from the last accepted click, not the last ignored one.
        assert!(!debounce.accept(start + Duration::from_millis(500)));
        assert!(debounce.accept(start + Duration::from_millis(600)));
    }

    #[test]
    fn a_time_going_backwards_is_ignored_not_a_panic() {
        let start = Instant::now();
        let mut debounce = Debounce::new(CLICK_WINDOW);
        assert!(debounce.accept(start + Duration::from_secs(1)));
        assert!(!debounce.accept(start));
    }

    #[test]
    fn a_toggle_right_after_a_hide_is_blocked() {
        let start = Instant::now();
        let mut hide = RecentHide::default();
        assert!(!hide.blocks_toggle(start), "never hidden");
        hide.mark(start);
        assert!(hide.blocks_toggle(start + Duration::from_millis(299)));
        assert!(!hide.blocks_toggle(start + Duration::from_millis(300)));
    }
}
