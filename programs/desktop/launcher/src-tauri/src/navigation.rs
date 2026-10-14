//! Where a launcher window may navigate to. Every window shows the app's own pages and nothing
//! else: a link, a redirect or a script that tries to load another page (or open a new window)
//! is refused, so the webview with its IPC access never shows foreign content.
//!
//! Tauri serves the bundled UI on Windows from `http://tauri.localhost/` on the default port. The
//! `https` variant exists only for windows built with `use_https_scheme(true)`; every window here
//! is built by `window::portable_builder`, which does not set it, so `https` is refused.
//! `tauri dev` serves the UI from the Vite dev server on port 1420.

/// The Vite dev server's port (`tauri.conf.json` `build.devUrl`).
pub const DEV_PORT: u16 = 1420;

/// Whether a window may navigate to `scheme://host:port`. `dev` allows the dev server too (debug
/// builds); a release build allows only the app's own origin and `about:blank`.
pub fn is_allowed(scheme: &str, host: Option<&str>, port: Option<u16>, dev: bool) -> bool {
    match (scheme, host) {
        ("about", None) => true,
        ("http", Some(host)) if host.eq_ignore_ascii_case("tauri.localhost") => port.is_none(),
        ("http", Some(host)) => {
            dev && host.eq_ignore_ascii_case("localhost") && port == Some(DEV_PORT)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_apps_own_origin_is_allowed() {
        assert!(is_allowed("http", Some("tauri.localhost"), None, false));
        assert!(is_allowed("http", Some("TAURI.localhost"), None, false));
    }

    #[test]
    fn the_app_origin_is_http_on_the_default_port_only() {
        // The windows are built in code with Tauri's default `http` scheme on Windows, so the
        // https variant is not the app and is refused, as is any explicit port.
        for dev in [false, true] {
            assert!(!is_allowed("https", Some("tauri.localhost"), None, dev));
            assert!(!is_allowed(
                "https",
                Some("tauri.localhost"),
                Some(443),
                dev
            ));
            assert!(!is_allowed(
                "http",
                Some("tauri.localhost"),
                Some(8080),
                dev
            ));
            assert!(!is_allowed("http", Some("tauri.localhost"), Some(80), dev));
        }
    }

    #[test]
    fn the_dev_server_is_allowed_only_in_dev() {
        assert!(is_allowed("http", Some("localhost"), Some(DEV_PORT), true));
        assert!(!is_allowed(
            "http",
            Some("localhost"),
            Some(DEV_PORT),
            false
        ));
        assert!(!is_allowed("http", Some("localhost"), Some(8080), true));
        assert!(!is_allowed("http", Some("localhost"), None, true));
    }

    #[test]
    fn everything_else_is_refused() {
        for (scheme, host, port) in [
            ("https", Some("example.com"), None),
            ("http", Some("tauri.localhost.evil.com"), None),
            ("http", Some("evil.com"), Some(1420)),
            ("file", None, None),
            ("javascript", None, None),
            ("data", None, None),
            ("ftp", Some("tauri.localhost"), None),
            ("launcher-icon", Some("localhost"), None),
            ("http", Some("ipc.localhost"), None),
            ("http", None, None),
        ] {
            assert!(!is_allowed(scheme, host, port, true), "{scheme} {host:?}");
            assert!(!is_allowed(scheme, host, port, false), "{scheme} {host:?}");
        }
    }

    #[test]
    fn a_blank_page_is_harmless() {
        assert!(is_allowed("about", None, None, false));
    }
}
