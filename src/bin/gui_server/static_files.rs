use std::path::{Component, Path, PathBuf};

use axum::body::Body;
use axum::extract::State;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

use super::AppState;

pub async fn serve(State(state): State<AppState>, uri: Uri) -> Response {
    let Some(relative) = request_path(uri.path(), &state.base_path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let candidate = state.static_root.join(&relative);
    match tokio::fs::read(&candidate).await {
        Ok(bytes) => response_for(&relative, bytes),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn request_path(path: &str, base_path: &str) -> Option<PathBuf> {
    let stripped = if base_path == "/" {
        path
    } else {
        path.strip_prefix(base_path).unwrap_or(path)
    };
    let stripped = stripped.trim_start_matches('/');
    let mut relative = PathBuf::from(stripped);
    if stripped.is_empty() || stripped.ends_with('/') {
        relative.push("index.html");
    }
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(relative)
}

fn response_for(path: &Path, bytes: Vec<u8>) -> Response {
    let content_type = match path.extension().and_then(|extension| extension.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("ico") => "image/x-icon",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") | Some("map") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml; charset=utf-8",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    };
    let cache_control = if is_next_static(path) {
        "public, max-age=31536000, immutable"
    } else {
        "no-store"
    };
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, cache_control),
        ],
        Body::from(bytes),
    )
        .into_response()
}

fn is_next_static(path: &Path) -> bool {
    let mut components = path.components();
    matches!(components.next(), Some(Component::Normal(value)) if value == "_next")
        && matches!(components.next(), Some(Component::Normal(value)) if value == "static")
        && components.next().is_some()
}
