//! `launcher-icon://localhost/<source>/<key>` (`http://launcher-icon.localhost/…` on Windows):
//! app icons looked up **by id** in the catalog. The UI never sends a file path.

// cspell:ignore nosniff

use std::borrow::Cow;

use genslate_launcher_core::catalog::load_icon;
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{Manager, Runtime, UriSchemeContext};

use crate::error::AppError;
use crate::icon_id::parse_id;
use crate::state::Launcher;

/// The URI scheme name.
pub const SCHEME: &str = "launcher-icon";

/// Handles one icon request.
pub fn handle<R: Runtime>(
    context: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
) -> Response<Cow<'static, [u8]>> {
    let launcher = context.app_handle().state::<Launcher>();
    let Some(id) = parse_id(request.uri().path()) else {
        return status(StatusCode::BAD_REQUEST);
    };
    let source = launcher
        .catalog()
        .find(&id)
        .and_then(|entry| entry.icon.clone());
    let Some(source) = source else {
        return status(StatusCode::NOT_FOUND);
    };
    match load_icon(&source, &launcher.paths.cache_dir) {
        Ok(icon) => image_response(icon.mime, icon.bytes),
        Err(error) => {
            log::debug!("icon {id}: {}", AppError::from(error).message());
            status(StatusCode::NOT_FOUND)
        }
    }
}

/// The response for an icon: its bytes with the type the catalog found, cacheable for five
/// minutes, and `nosniff` so the webview never reinterprets them as anything but an image.
fn image_response(mime: &'static str, bytes: Vec<u8>) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "max-age=300")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(Cow::Owned(bytes))
        .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
}

fn status(code: StatusCode) -> Response<Cow<'static, [u8]>> {
    let mut response = Response::new(Cow::Borrowed(&[][..]));
    *response.status_mut() = code;
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_icon_response_is_nosniff_and_keeps_its_type() {
        let response = image_response("image/svg+xml", b"<svg/>".to_vec());
        let header_text = |name| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
        };
        assert_eq!(header_text(header::X_CONTENT_TYPE_OPTIONS), Some("nosniff"));
        assert_eq!(header_text(header::CONTENT_TYPE), Some("image/svg+xml"));
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body().as_ref(), b"<svg/>");
    }
}
