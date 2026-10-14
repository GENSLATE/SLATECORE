//! The launcher window: a fixed-size transparent window anchored to the bottom-right corner of
//! the work area of the monitor under the cursor. It cannot be moved.
//!
//! The window is always as wide as the *expanded* (tools) layout; the UI draws the frame at the
//! right edge and animates it in CSS, so showing, hiding and widening are pure transform and
//! opacity animations (Tauri cannot move and resize atomically, which would flash). Nothing
//! drags the window: it has no title bar, the webview holds no drag permission, and when
//! something else moves it anyway (Win+Shift+Arrow, a display change) [`keep_in_place`] puts it
//! back. The transparent margin is made click-through by [`spawn_hit_test`].
//!
//! Every window of the app, this one and the tray menu, is built by [`portable_builder`], which
//! is the one place that sets the webview's data folder, incognito mode, no shadow and no
//! taskbar button.

// cspell:ignore minimizable
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use genslate_launcher_core::geometry::{self, Rect};
use genslate_paths::AppPaths;
use serde::{Deserialize, Serialize};
use tauri::window::Color;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

use tauri::webview::NewWindowResponse;

use crate::brand::WINDOW_TITLE;
use crate::debounce::{ToggleAction, ToggleState, decide_toggle};
use crate::error::AppError;
use crate::events;
use crate::navigation;
use crate::placement::{self, MonitorInfo};
use crate::state::{FrameGeometry, Launcher};

/// Label of the launcher window.
pub const MAIN: &str = "main";
/// How long the UI's exit animation gets before the native window hides.
const HIDE_DELAY: Duration = Duration::from_millis(140);
/// The window and its webview are fully transparent: the UI draws the frame. This is not a theme
/// colour, so it is not a design token.
const TRANSPARENT: Color = Color(0, 0, 0, 0);

/// A window builder with everything the leave-no-trace contract needs, shared by every window:
/// the webview's data folder is inside installDir (and is the same for every window), the
/// webview is incognito, there is no shadow and no taskbar button, and the window is frameless,
/// transparent, fixed in size and hidden until placed. Every window may show only the app's own
/// pages: other navigations are refused and no window can open another ([`navigation`]).
pub fn portable_builder<'a, R: Runtime>(
    app: &'a AppHandle<R>,
    label: &str,
    url: WebviewUrl,
    paths: &AppPaths,
) -> WebviewWindowBuilder<'a, R, AppHandle<R>> {
    WebviewWindowBuilder::new(app, label, url)
        .data_directory(paths.webview2_dir.clone())
        .incognito(true)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .visible(false)
        .background_color(TRANSPARENT)
        .on_navigation(|url| {
            let allowed = navigation::is_allowed(
                url.scheme(),
                url.host_str(),
                url.port(),
                cfg!(debug_assertions),
            );
            if !allowed {
                log::warn!("a window tried to navigate to a page that is not the app's");
            }
            allowed
        })
        .on_new_window(|_url, _features| {
            log::warn!("a window tried to open a new window");
            NewWindowResponse::Deny
        })
}

/// Creates the (hidden) launcher window.
pub fn create<R: Runtime>(
    app: &AppHandle<R>,
    launcher: &Launcher,
) -> Result<WebviewWindow<R>, AppError> {
    let size = launcher.settings().config.appearance.size;
    let (width, height) = geometry::window_size(size);
    let pinned = launcher.window().pinned;
    let window = portable_builder(app, MAIN, WebviewUrl::default(), &launcher.paths)
        .title(WINDOW_TITLE)
        .inner_size(width, height)
        .always_on_top(pinned)
        .build()?;
    Ok(window)
}

/// The launcher window, or an error if it is gone.
pub fn main_window<R: Runtime>(app: &AppHandle<R>) -> Result<WebviewWindow<R>, AppError> {
    app.get_webview_window(MAIN)
        .ok_or(AppError::MissingWindow(MAIN))
}

/// The view the UI opens on when the window shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShowView {
    /// The apps list (every regular show).
    #[default]
    Apps,
    /// Shortcuts and commands (the tray menu's Help).
    Help,
    /// The Settings tool.
    Settings,
}

fn rect(position: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> Rect {
    Rect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    }
}

/// Every monitor with its work area and scale factor.
pub fn monitors<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<MonitorInfo>, AppError> {
    Ok(app
        .available_monitors()?
        .iter()
        .map(|monitor| {
            let work = monitor.work_area();
            MonitorInfo {
                screen: rect(*monitor.position(), *monitor.size()),
                work: rect(work.position, work.size),
                scale: monitor.scale_factor(),
            }
        })
        .collect())
}

/// Moves the window, then sizes it, then moves it again: the first move makes it adopt the
/// target monitor's DPI, the size is then in that monitor's pixels, and the last move settles
/// the corner.
fn place<R: Runtime>(window: &WebviewWindow<R>, target: Rect) -> tauri::Result<()> {
    let position = PhysicalPosition::new(target.x, target.y);
    window.set_position(position)?;
    window.set_size(PhysicalSize::new(target.width, target.height))?;
    window.set_position(position)
}

/// [`place`], then a look at the size the window really has: moving a window to a monitor with
/// another scale factor can make Windows rescale it on arrival, after our `set_size`, and the
/// first pass then leaves it at the wrong size. When the size is off, place it once more (the
/// window now lives on the target monitor, so the same physical size sticks). Done before the
/// window is shown, so nobody sees the correction. UNVERIFIED on a mixed-DPI setup.
pub fn place_settled<R: Runtime>(window: &WebviewWindow<R>, target: Rect) -> tauri::Result<()> {
    place(window, target)?;
    let size = window.outer_size()?;
    if !placement::size_settled((size.width, size.height), target) {
        place(window, target)?;
    }
    Ok(())
}

/// Places the window on the monitor under the cursor, shows and focuses it, then tells the UI to
/// play its entrance on the apps list.
pub fn show<R: Runtime>(app: &AppHandle<R>) -> Result<(), AppError> {
    show_view(app, ShowView::Apps)
}

/// Like [`show`], opening on `view`.
pub fn show_view<R: Runtime>(app: &AppHandle<R>, view: ShowView) -> Result<(), AppError> {
    let window = main_window(app)?;
    let launcher = app.state::<Launcher>();
    let size = launcher.settings().config.appearance.size;
    let cursor = app
        .cursor_position()
        .map_or((0.0, 0.0), |position| (position.x, position.y));
    let target = placement::target_rect(&monitors(app)?, cursor, geometry::window_size(size));
    if let Some((target, scale)) = target {
        place_settled(&window, target)?;
        launcher.window().geometry = Some(FrameGeometry {
            window: target,
            scale,
        });
    }
    window.show()?;
    window.set_focus()?;
    launcher.window().visible = true;
    if let Some(service) = launcher.vault_service() {
        service.set_visible(true);
    }
    app.emit(events::SHOWN, view)?;
    Ok(())
}

/// Plays the UI's exit animation, then hides the native window.
pub fn hide<R: Runtime>(app: &AppHandle<R>) -> Result<(), AppError> {
    let launcher = app.state::<Launcher>();
    {
        let mut flags = launcher.window();
        if !flags.visible {
            return Ok(());
        }
        flags.visible = false;
        flags.recent_hide.mark(Instant::now());
    }
    if let Some(service) = launcher.vault_service() {
        service.set_visible(false);
    }
    app.emit(events::WILL_HIDE, ())?;
    let handle = app.clone();
    thread::spawn(move || {
        thread::sleep(HIDE_DELAY);
        let launcher = handle.state::<Launcher>();
        // Shown again during the animation? Then stay.
        if launcher.is_visible() {
            return;
        }
        if let Err(error) = main_window(&handle).and_then(|window| Ok(window.hide()?)) {
            log::warn!("could not hide the launcher: {}", error.message());
        }
    });
    Ok(())
}

/// Tray click or global hotkey: hide when visible and focused (or when this very click took the
/// focus away from a launcher a blur does not hide), otherwise show. A toggle right after a hide
/// is the second half of the click that hid the window (the click blurred it), so it does
/// nothing. The decision itself is [`decide_toggle`].
pub fn toggle<R: Runtime>(app: &AppHandle<R>) -> Result<(), AppError> {
    let launcher = app.state::<Launcher>();
    let now = Instant::now();
    let (visible, just_hidden, just_blurred) = {
        let flags = launcher.window();
        (
            flags.visible,
            flags.recent_hide.blocks_toggle(now),
            flags.recent_blur.is_recent(now),
        )
    };
    let focused = main_window(app)?.is_focused().unwrap_or(false);
    match decide_toggle(ToggleState {
        visible,
        focused,
        just_hidden,
        just_blurred,
    }) {
        ToggleAction::Nothing => Ok(()),
        ToggleAction::Hide => hide(app),
        ToggleAction::Show => show(app),
    }
}

/// Pins (always on top, never auto-hides) or unpins the window.
pub fn set_pinned<R: Runtime>(app: &AppHandle<R>, pinned: bool) -> Result<(), AppError> {
    main_window(app)?.set_always_on_top(pinned)?;
    app.state::<Launcher>().window().pinned = pinned;
    app.emit(events::PINNED, pinned)?;
    Ok(())
}

/// Resizes for a new height preset (the width never changes). A visible window is placed again
/// in its corner of the monitor it is on, at the new size, and nothing else happens: no
/// `launcher://shown` event, so the UI keeps the view it is showing (Settings, usually, since
/// the size is changed there).
pub fn apply_size<R: Runtime>(app: &AppHandle<R>) -> Result<(), AppError> {
    let launcher = app.state::<Launcher>();
    if launcher.is_visible() {
        snap_back(app, &launcher)
    } else {
        let (width, height) = geometry::window_size(launcher.settings().config.appearance.size);
        main_window(app)?.set_size(tauri::LogicalSize::new(width, height))?;
        Ok(())
    }
}

/// Hides on focus loss unless pinned or turned off in `settings.toml`.
pub fn on_blur<R: Runtime>(app: &AppHandle<R>) {
    let launcher = app.state::<Launcher>();
    // Always, hidden or not: a tray click that follows means "hide" for a launcher a blur leaves
    // on screen.
    launcher.window().recent_blur.mark(Instant::now());
    let pinned = launcher.window().pinned;
    let enabled = launcher.settings().config.behavior.hide_on_blur;
    if !pinned
        && enabled
        && let Err(error) = hide(app)
    {
        log::warn!("could not hide on blur: {}", error.message());
    }
}

/// Puts a visible window back in its corner when something moved or resized it (a drag by some
/// other tool, Win+Shift+Arrow, a DPI change on another monitor). A window already in place is
/// left alone, which is also what ends the loop of our own moves raising more events.
pub fn keep_in_place<R: Runtime>(app: &AppHandle<R>) {
    let launcher = app.state::<Launcher>();
    if !launcher.is_visible() {
        return;
    }
    if let Err(error) = snap_back(app, &launcher) {
        log::warn!("could not keep the launcher in place: {}", error.message());
    }
}

fn snap_back<R: Runtime>(app: &AppHandle<R>, launcher: &Launcher) -> Result<(), AppError> {
    let window = main_window(app)?;
    let actual = rect(window.outer_position()?, window.outer_size()?);
    let size = launcher.settings().config.appearance.size;
    let corner = placement::snap_back(actual, &monitors(app)?, geometry::window_size(size));
    let frame = match corner {
        Some((target, scale)) => {
            place_settled(&window, target)?;
            FrameGeometry {
                window: target,
                scale,
            }
        }
        None => FrameGeometry {
            window: actual,
            scale: window.scale_factor()?,
        },
    };
    launcher.window().geometry = Some(frame);
    Ok(())
}

/// Keeps the transparent part of the window click-through: about 60 times a second while the
/// window is visible, the cursor is tested against the frame the UI draws at the right edge. The
/// window's rectangle comes from the cache [`show_view`] and [`keep_in_place`] keep, so a poll
/// asks the window nothing.
pub fn spawn_hit_test<R: Runtime>(app: AppHandle<R>) {
    let spawned = thread::Builder::new()
        .name("launcher-hit-test".to_owned())
        .spawn(move || {
            let mut passing_through: Option<bool> = None;
            loop {
                let launcher = app.state::<Launcher>();
                let (visible, expanded, geometry) = {
                    let flags = launcher.window();
                    (flags.visible, flags.expanded, flags.geometry)
                };
                if !visible {
                    passing_through = None;
                    thread::sleep(Duration::from_millis(120));
                    continue;
                }
                let inside = launcher.popup_open() || cursor_in_frame(&app, geometry, expanded);
                if passing_through != Some(!inside)
                    && let Ok(window) = main_window(&app)
                    && window.set_ignore_cursor_events(!inside).is_ok()
                {
                    passing_through = Some(!inside);
                }
                thread::sleep(Duration::from_millis(16));
            }
        });
    if let Err(error) = spawned {
        log::error!("click-through thread failed to start: {error}");
    }
}

/// Whether the cursor is over the visible frame. When the position of either is not known the
/// answer is yes: a window that takes clicks it should not is better than one that loses them.
fn cursor_in_frame<R: Runtime>(
    app: &AppHandle<R>,
    geometry: Option<FrameGeometry>,
    expanded: bool,
) -> bool {
    let (Some(frame), Ok(cursor)) = (geometry, app.cursor_position()) else {
        return true;
    };
    placement::cursor_in_frame(frame.window, frame.scale, expanded, (cursor.x, cursor.y))
}

/// Marks the expanded (tools) layout: the UI animates, the shell widens the hit area.
pub fn set_expanded<R: Runtime>(app: &AppHandle<R>, expanded: bool) {
    app.state::<Launcher>().window().expanded = expanded;
}

/// Records that a popup is open (menus may reach over the transparent area).
pub fn set_popup<R: Runtime>(app: &AppHandle<R>, open: bool) {
    app.state::<Launcher>()
        .popup_open
        .store(open, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_views_use_the_frontends_spellings() -> Result<(), serde_json::Error> {
        assert_eq!(serde_json::to_string(&ShowView::Apps)?, "\"apps\"");
        assert_eq!(serde_json::to_string(&ShowView::Help)?, "\"help\"");
        assert_eq!(serde_json::to_string(&ShowView::Settings)?, "\"settings\"");
        assert_eq!(ShowView::default(), ShowView::Apps);
        Ok(())
    }
}
