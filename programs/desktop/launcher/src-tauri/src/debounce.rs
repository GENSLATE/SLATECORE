//! Click debouncing for the tray, free of Tauri types so it is tested anywhere.
//!
//! Windows delivers a tray click as down, up (and a double click as down, up, double-click, up),
//! and the click moves focus to the taskbar, which blurs the launcher before the "up" arrives.
//! Three small guards deal with it: [`Debounce`] drops clicks that follow another accepted click
//! too closely; [`RecentHide`] tells the toggle that the launcher was hidden a moment ago, by the
//! very click that is now asking to show it (the blur hid it); and [`RecentBlur`] covers the
//! launcher that a blur does *not* hide (pinned, or hide-on-blur turned off): it is still
//! visible but no longer focused when the click arrives, and that click means "hide it".
//! [`decide_toggle`] puts the three together.

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

/// Remembers when the launcher last lost focus.
#[derive(Debug, Clone, Default)]
pub struct RecentBlur {
    blurred_at: Option<Instant>,
}

impl RecentBlur {
    /// Records that focus left the launcher at `now`.
    pub fn mark(&mut self, now: Instant) {
        self.blurred_at = Some(now);
    }

    /// `true` while a toggle at `now` can still be the click that took the focus away.
    pub fn is_recent(&self, now: Instant) -> bool {
        self.blurred_at
            .is_some_and(|at| now.saturating_duration_since(at) < CLICK_WINDOW)
    }
}

/// What a tray click or the global shortcut does to the launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleAction {
    /// The second half of a click that already hid the launcher.
    Nothing,
    Hide,
    Show,
}

/// What is known about the launcher when a toggle arrives.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "four independent facts about one instant; an enum would only rename them"
)]
pub struct ToggleState {
    /// The window is shown.
    pub visible: bool,
    /// It has the keyboard focus.
    pub focused: bool,
    /// It was hidden less than a click window ago ([`RecentHide`]).
    pub just_hidden: bool,
    /// It lost focus less than a click window ago ([`RecentBlur`]).
    pub just_blurred: bool,
}

/// Hide when the launcher is visible and either focused or blurred by this very click, show
/// otherwise, and do nothing right after a hide.
pub fn decide_toggle(state: ToggleState) -> ToggleAction {
    if state.just_hidden {
        ToggleAction::Nothing
    } else if state.visible && (state.focused || state.just_blurred) {
        ToggleAction::Hide
    } else {
        ToggleAction::Show
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

    #[test]
    fn a_blur_is_recent_for_the_click_window_only() {
        let start = Instant::now();
        let mut blur = RecentBlur::default();
        assert!(!blur.is_recent(start), "never blurred");
        blur.mark(start);
        assert!(blur.is_recent(start + Duration::from_millis(299)));
        assert!(!blur.is_recent(start + Duration::from_millis(300)));
    }

    #[test]
    fn a_focused_visible_launcher_hides() {
        let state = ToggleState {
            visible: true,
            focused: true,
            ..ToggleState::default()
        };
        assert_eq!(decide_toggle(state), ToggleAction::Hide);
    }

    #[test]
    fn a_hidden_launcher_shows() {
        assert_eq!(decide_toggle(ToggleState::default()), ToggleAction::Show);
        // Hidden but "recently blurred" (the blur that hid it) is a show, unless just hidden.
        let state = ToggleState {
            just_blurred: true,
            ..ToggleState::default()
        };
        assert_eq!(decide_toggle(state), ToggleAction::Show);
    }

    #[test]
    fn a_visible_launcher_that_lost_focus_a_while_ago_shows_and_takes_focus() {
        let state = ToggleState {
            visible: true,
            ..ToggleState::default()
        };
        assert_eq!(decide_toggle(state), ToggleAction::Show);
    }

    #[test]
    fn a_click_right_after_the_blur_hides_a_pinned_launcher() {
        // Pinned, or hide-on-blur turned off: the click blurs the launcher but does not hide it,
        // so the toggle finds it visible and unfocused. That is "hide", not "show".
        let start = Instant::now();
        let mut blur = RecentBlur::default();
        let hide = RecentHide::default();
        blur.mark(start);
        let state_at = |now: Instant| ToggleState {
            visible: true,
            focused: false,
            just_hidden: hide.blocks_toggle(now),
            just_blurred: blur.is_recent(now),
        };
        let action = decide_toggle(state_at(start + Duration::from_millis(40)));
        assert_eq!(action, ToggleAction::Hide);
        // The same press a second later is a new gesture on a launcher left unfocused.
        let action = decide_toggle(state_at(start + Duration::from_secs(1)));
        assert_eq!(action, ToggleAction::Show);
    }

    #[test]
    fn the_toggle_after_the_blur_that_hid_the_launcher_does_nothing() {
        let start = Instant::now();
        let mut blur = RecentBlur::default();
        let mut hide = RecentHide::default();
        blur.mark(start);
        hide.mark(start);
        let at = start + Duration::from_millis(40);
        let action = decide_toggle(ToggleState {
            visible: false,
            focused: false,
            just_hidden: hide.blocks_toggle(at),
            just_blurred: blur.is_recent(at),
        });
        assert_eq!(action, ToggleAction::Nothing);
    }
}
