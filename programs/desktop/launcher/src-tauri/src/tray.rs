//! The tray icon. A left click toggles the launcher; a right click opens the tray menu.
//!
//! The Windows notification area can report one click twice (button up on each of two windows)
//! and a click on the icon first blurs the open launcher, which hides it, and then toggles. The
//! first is [`Debounce`]'s job, the second `window::toggle`'s (it ignores a toggle that follows a
//! hide by less than the click window).

use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use tauri::AppHandle;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

use crate::brand::TRAY_TOOLTIP;
use crate::debounce::CLICK_WINDOW;
pub use crate::debounce::Debounce;
use crate::error::AppError;
use crate::{tray_menu, window};

/// Id of the tray icon.
pub const TRAY_ID: &str = "main";

/// Builds the tray icon (kept alive by Tauri under [`TRAY_ID`]).
pub fn create(app: &AppHandle) -> Result<(), AppError> {
    let clicks = Mutex::new(Debounce::new(CLICK_WINDOW));
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(TRAY_TOOLTIP)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(move |tray, event| {
            let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            else {
                return;
            };
            let app = tray.app_handle();
            let result = match button {
                MouseButton::Left => {
                    let fresh = clicks
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .accept(Instant::now());
                    if fresh { window::toggle(app) } else { Ok(()) }
                }
                MouseButton::Right => tray_menu::open(app, position),
                MouseButton::Middle => Ok(()),
            };
            if let Err(error) = result {
                log::warn!("tray: {}", error.message());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
