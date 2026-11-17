//! The web app's files, compiled into the binary (release) or read from ./web (debug).

use crate::AppState;
use axum::Json;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;
use serde_json::json;
use std::sync::Arc;

#[derive(RustEmbed)]
#[folder = "web/"]
struct Web;

/// Theme and language from the small, non-secret preference cookie `tuv_prefs=dark.en`.
/// Only known values pass, so the cookie cannot inject anything into the page.
fn prefs(req: &Request) -> (&'static str, &'static str) {
    let mut theme = "auto";
    let mut lang = "bg";
    for v in req.headers().get_all(header::COOKIE) {
        let Ok(s) = v.to_str() else { continue };
        for part in s.split(';') {
            if let Some(val) = part.trim().strip_prefix("tuv_prefs=") {
                let mut it = val.split('.');
                theme = match it.next() {
                    Some("dark") => "dark",
                    Some("light") => "light",
                    _ => "auto",
                };
                lang = match it.next() {
                    Some("en") => "en",
                    _ => "bg",
                };
            }
        }
    }
    (theme, lang)
}

pub async fn serve(State(_st): State<Arc<AppState>>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    if path.starts_with("/api/") || path.starts_with("/auth/") {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "not_found"}))).into_response();
    }
    let p = path.trim_start_matches('/');
    let p = if p.is_empty() { "index.html" } else { p };
    if p.contains("..") || p.contains('\\') {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(file) = Web::get(p) else {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    };
    let is_index = p == "index.html";
    let (theme, lang) = if is_index { prefs(&req) } else { ("", "") };
    let etag = format!(
        "\"{}{}{}\"",
        &crate::util::hex(&file.metadata.sha256_hash())[..20],
        theme,
        lang
    );
    if req
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        == Some(etag.as_str())
    {
        let mut r = StatusCode::NOT_MODIFIED.into_response();
        r.headers_mut()
            .insert(header::ETAG, HeaderValue::from_str(&etag).expect("ascii"));
        return r;
    }
    let body: Vec<u8> = if is_index {
        String::from_utf8_lossy(&file.data)
            .replacen("data-theme=\"auto\"", &format!("data-theme=\"{theme}\""), 1)
            .replacen("<html lang=\"bg\"", &format!("<html lang=\"{lang}\""), 1)
            .into_bytes()
    } else {
        file.data.into_owned()
    };
    let mime = match p.rsplit('.').next() {
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8".to_string(),
        Some("webmanifest") => "application/manifest+json".to_string(),
        Some("html") => "text/html; charset=utf-8".to_string(),
        Some("css") => "text/css; charset=utf-8".to_string(),
        Some("svg") => "image/svg+xml".to_string(),
        _ => mime_guess::from_path(p).first_or_octet_stream().to_string(),
    };
    let mut r = body.into_response();
    let h = r.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    h.insert(header::ETAG, HeaderValue::from_str(&etag).expect("ascii"));
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if is_index || p == "sw.js" {
            "no-cache"
        } else {
            "public, max-age=3600"
        }),
    );
    if is_index {
        h.insert(header::VARY, HeaderValue::from_static("Cookie"));
    }
    r
}
