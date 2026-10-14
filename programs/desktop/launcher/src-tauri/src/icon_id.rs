//! The icon requests of the `launcher-icon:` URI scheme name an app by id. This turns the
//! request path into an [`AppId`] and nothing else: the UI never sends a file path, and the
//! file is looked up in the catalog. No Tauri types, so it is tested on any host.

use genslate_launcher_core::catalog::AppId;

/// `/genslate/explorer` or `/portableapps/LibreOfficePortable%232` becomes an [`AppId`]; any
/// other shape (extra segments, `..`, bad escapes, unknown source) is `None`.
pub fn parse_id(path: &str) -> Option<AppId> {
    let mut parts = path.trim_start_matches('/').splitn(2, '/');
    let source = percent_decode(parts.next()?)?;
    let key = percent_decode(parts.next()?)?;
    format!("{source}/{key}").parse().ok()
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes.get(index) == Some(&b'%') {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(*bytes.get(index)?);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_encoded_ids() {
        assert_eq!(
            parse_id("/genslate/explorer")
                .map(|id| id.to_string())
                .as_deref(),
            Some("genslate/explorer")
        );
        assert_eq!(
            parse_id("/portableapps/LibreOfficePortable%232").map(|id| id.key),
            Some("LibreOfficePortable#2".to_owned())
        );
    }

    #[test]
    fn refuses_anything_that_is_not_one_source_and_one_key() {
        assert_eq!(parse_id("/genslate/..%2F..%2Fsecret"), None);
        assert_eq!(parse_id("/genslate/a%5Cb"), None);
        assert_eq!(parse_id("/nope/x"), None);
        assert_eq!(parse_id("/genslate/%ZZ"), None);
        assert_eq!(parse_id("/genslate/%2"), None);
        assert_eq!(parse_id("/genslate"), None);
        assert_eq!(parse_id(""), None);
    }
}
