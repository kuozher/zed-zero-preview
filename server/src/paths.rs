use std::path::PathBuf;

/// Percent-encode a path for `/raw/` URLs.
///
/// Ported from `zlp-src/cli.js` `encodePathForUrl` plus the normalization
/// that strips a leading slash (and a leading slash before a Windows drive)
/// so `/raw/` prefixes compose uniformly.
pub fn encode_path_for_url(path: &str) -> String {
    let mut norm = path.replace('\\', "/");
    if has_leading_drive_slash(&norm) {
        norm.remove(0);
    }
    if norm.starts_with('/') {
        norm.remove(0);
    }

    norm.split('/')
        .enumerate()
        .map(|(idx, seg)| {
            if idx == 0 && is_windows_drive(seg) {
                seg.to_string()
            } else {
                encode_uri_component(seg)
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Decode a `/raw/` URL remainder into a filesystem path.
///
/// Ported from `zlp-src/server.js` `/raw/` handling. `is_windows` is injected
/// so tests can cover both platforms; runtime callers pass `cfg!(windows)`.
pub fn raw_url_to_fs_path(raw: &str, is_windows: bool) -> PathBuf {
    let mut raw_path = decode_uri_component(raw);
    if is_windows {
        if has_leading_drive_slash(&raw_path) {
            raw_path.remove(0);
        }
    } else if !starts_with_drive(&raw_path) && !raw_path.starts_with('/') {
        raw_path.insert(0, '/');
    }
    PathBuf::from(raw_path)
}

/// Decode a URL path component (e.g. request pathname for referer fallback).
pub fn decode_path_component(s: &str) -> String {
    decode_uri_component(s)
}

/// Convert an LSP `file://` URI into a filesystem path.
pub fn file_uri_to_path(uri: &str, is_windows: bool) -> PathBuf {
    let rest = uri.strip_prefix("file://").unwrap_or(uri);
    raw_url_to_fs_path(rest, is_windows)
}

/// `encodeURIComponent` unreserved set: `A-Z a-z 0-9 - _ . ! ~ * ' ( )`
fn is_uri_unreserved(b: u8) -> bool {
    matches!(
        b,
        b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')'
    )
}

fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if is_uri_unreserved(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decode_uri_component(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn is_windows_drive(seg: &str) -> bool {
    let b = seg.as_bytes();
    b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

fn starts_with_drive(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

fn has_leading_drive_slash(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_segments_keeping_windows_drive() {
        assert_eq!(encode_path_for_url("C:/我的 專案/a#b/100%.html"),
                   "C:/%E6%88%91%E7%9A%84%20%E5%B0%88%E6%A1%88/a%23b/100%25.html");
    }
    #[test]
    fn encodes_posix_path_with_special_chars() {
        assert_eq!(encode_path_for_url("home/eric/a+b/c&d (1).html"),
                   "home/eric/a%2Bb/c%26d%20(1).html");
    }
    #[test]
    fn decodes_raw_url_to_windows_path() {
        assert_eq!(raw_url_to_fs_path("/C:/%E6%B8%AC%E8%A9%A6/a%23b.html", true),
                   std::path::PathBuf::from("C:/測試/a#b.html"));
    }
    #[test]
    fn decodes_raw_url_to_posix_path() {
        assert_eq!(raw_url_to_fs_path("home/eric/a%20b.html", false),
                   std::path::PathBuf::from("/home/eric/a b.html"));
    }
    #[test]
    fn file_uri_to_path_handles_windows_drive() {
        assert_eq!(file_uri_to_path("file:///C:/dir/%E4%B8%AD%E6%96%87.html", true),
                   std::path::PathBuf::from("C:/dir/中文.html"));
    }
    #[test]
    fn file_uri_to_path_posix() {
        assert_eq!(file_uri_to_path("file:///home/e/x.html", false),
                   std::path::PathBuf::from("/home/e/x.html"));
    }
}
