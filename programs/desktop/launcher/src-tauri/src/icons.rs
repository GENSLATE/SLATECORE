//! `launcher-icon://localhost/<source>/<key>` (`http://launcher-icon.localhost/…` on Windows):
//! app icons looked up **by id** in the catalog. The UI never sends a file path.

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
        Ok(icon) => Response::builder()
            .header(header::CONTENT_TYPE, icon.mime)
            .header(header::CACHE_CONTROL, "max-age=300")
            .body(Cow::Owned(icon.bytes))
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
        Err(error) => {
            log::debug!("icon {id}: {}", AppError::from(error).message());
            status(StatusCode::NOT_FOUND)
        }
    }
}

fn status(code: StatusCode) -> Response<Cow<'static, [u8]>> {
    let mut response = Response::new(Cow::Borrowed(&[][..]));
    *response.status_mut() = code;
    response
}
