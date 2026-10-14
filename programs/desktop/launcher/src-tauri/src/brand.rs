//! The product name as the shell shows it (window titles, tray tooltip, dialogs).
//!
//! Brand text is always in capitals (`.claude/rules/branding.md`); the code namespace stays
//! lowercase GENSLATE. Kept apart from the Tauri modules so a test can check it everywhere.

/// Window title, tray tooltip and the title of every message box.
pub const PRODUCT: &str = "SLATECORE LAUNCHER";
/// Title of the launcher window.
pub const WINDOW_TITLE: &str = PRODUCT;
/// Title of the (hidden) tray menu window.
pub const TRAY_MENU_TITLE: &str = "SLATECORE LAUNCHER menu";
/// Tooltip of the tray icon (the Win32 limit is 127 characters).
pub const TRAY_TOOLTIP: &str = PRODUCT;

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = include_str!("../tauri.conf.json");

    #[test]
    fn window_title_and_tray_tooltip_say_slatecore_launcher() {
        assert_eq!(WINDOW_TITLE, "SLATECORE LAUNCHER");
        assert_eq!(TRAY_TOOLTIP, "SLATECORE LAUNCHER");
        assert!(TRAY_MENU_TITLE.starts_with("SLATECORE LAUNCHER"));
        let config: serde_json::Value = serde_json::from_str(CONFIG).unwrap_or_default();
        assert_eq!(config["productName"], WINDOW_TITLE);
        assert_eq!(config["identifier"], "xyz.genslate.slatecore.launcher");
        assert_eq!(config["mainBinaryName"], "slatecore-launcher");
    }
}
