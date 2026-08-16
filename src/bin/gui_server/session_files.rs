use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path as FilePath, PathBuf};

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use super::AppState;
use super::api::{
    MAX_LIST_ENTRIES, MAX_TEXT_BYTES, checked_existing_directory,
    checked_existing_path_without_symlinks, collect_documents, document, document_summary,
};
use super::sessions::{require_session_id, require_trial};

const MAX_EVENT_TAIL_LINES: usize = 2_000;
const TAIL_READ_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactQuery {
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventsQuery {
    tail: usize,
}

#[derive(Debug, Serialize)]
struct EventDocument {
    id: &'static str,
    path: &'static str,
    content: String,
}

#[derive(Debug)]
struct TailError {
    status: StatusCode,
    message: String,
}

impl IntoResponse for TailError {
    fn into_response(self) -> Response {
        json_error(self.status, self.message)
    }
}

pub async fn artifacts(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<ArtifactQuery>,
) -> Result<Response, Response> {
    let run_root = session_run_root(&state, &id, &headers).await?;
    if let Some(path) = query.path {
        let path = checked_existing_path_without_symlinks(&run_root, FilePath::new(&path))
            .await
            .map_err(IntoResponse::into_response)?;
        let value = document(&run_root, &path)
            .await
            .map_err(IntoResponse::into_response)?;
        return Ok(Json(value).into_response());
    }

    let documents = collect_documents(&run_root, 4)
        .await
        .map_err(IntoResponse::into_response)?;
    let summaries = documents
        .iter()
        .filter_map(|path| document_summary(&run_root, path))
        .take(MAX_LIST_ENTRIES)
        .collect::<Vec<_>>();
    Ok(Json(summaries).into_response())
}

pub async fn events(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<EventsQuery>,
) -> Result<Response, Response> {
    let run_root = session_run_root(&state, &id, &headers).await?;
    if !(1..=MAX_EVENT_TAIL_LINES).contains(&query.tail) {
        return Err(json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("tail must be in 1..={MAX_EVENT_TAIL_LINES}"),
        ));
    }
    let path = checked_existing_path_without_symlinks(&run_root, FilePath::new("events.jsonl"))
        .await
        .map_err(IntoResponse::into_response)?;
    let tail = query.tail;
    let content = tokio::task::spawn_blocking(move || read_event_tail(&path, tail))
        .await
        .map_err(|error| {
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("join event tail reader: {error}"),
            )
        })?
        .map_err(IntoResponse::into_response)?;
    Ok(Json(EventDocument {
        id: "events.jsonl",
        path: "events.jsonl",
        content,
    })
    .into_response())
}

async fn session_run_root(
    state: &AppState,
    id: &str,
    headers: &HeaderMap,
) -> Result<PathBuf, Response> {
    let workspace = require_trial(state, headers, false).map_err(IntoResponse::into_response)?;
    require_session_id(id).map_err(IntoResponse::into_response)?;
    let anvil_root = workspace.join(".anvil");
    let runs_root = anvil_root.join("runs");
    for directory in [&anvil_root, &runs_root] {
        let metadata = tokio::fs::symlink_metadata(directory)
            .await
            .map_err(|error| {
                json_error(
                    StatusCode::NOT_FOUND,
                    format!("session run root not found: {error}"),
                )
            })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(json_error(
                StatusCode::NOT_FOUND,
                "session run root symlinks are not readable",
            ));
        }
    }
    let run_root = checked_existing_directory(&runs_root, FilePath::new(id))
        .await
        .map_err(IntoResponse::into_response)?;
    let metadata = tokio::fs::symlink_metadata(&run_root)
        .await
        .map_err(|error| {
            json_error(
                StatusCode::NOT_FOUND,
                format!("session run not found: {error}"),
            )
        })?;
    if metadata.file_type().is_symlink() {
        return Err(json_error(
            StatusCode::NOT_FOUND,
            "session run symlinks are not readable",
        ));
    }
    Ok(run_root)
}

fn read_event_tail(path: &FilePath, line_limit: usize) -> Result<String, TailError> {
    let mut file = File::open(path).map_err(|error| {
        tail_error(
            StatusCode::NOT_FOUND,
            format!("read {}: {error}", path.display()),
        )
    })?;
    let length = file
        .metadata()
        .map_err(|error| {
            tail_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("inspect {}: {error}", path.display()),
            )
        })?
        .len();
    if length == 0 {
        return Ok(String::new());
    }

    let mut position = length;
    let mut chunks = Vec::new();
    let mut scanned_bytes = 0usize;
    let mut newline_count = 0usize;
    let mut trailing_newline = false;
    while position > 0 {
        let chunk_length = usize::try_from(position.min(TAIL_READ_CHUNK_BYTES as u64))
            .expect("tail chunk length fits usize");
        position -= chunk_length as u64;
        file.seek(SeekFrom::Start(position)).map_err(|error| {
            tail_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("seek {}: {error}", path.display()),
            )
        })?;
        let mut chunk = vec![0; chunk_length];
        file.read_exact(&mut chunk).map_err(|error| {
            tail_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("read {}: {error}", path.display()),
            )
        })?;
        if chunks.is_empty() {
            trailing_newline = chunk.last() == Some(&b'\n');
        }
        newline_count += chunk.iter().filter(|byte| **byte == b'\n').count();
        scanned_bytes += chunk.len();
        chunks.push(chunk);

        let boundary_count = line_limit + usize::from(trailing_newline);
        if newline_count >= boundary_count {
            break;
        }
        if scanned_bytes > MAX_TEXT_BYTES as usize + TAIL_READ_CHUNK_BYTES {
            return Err(tail_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "selected event tail exceeds the 1 MiB viewing limit",
            ));
        }
    }

    let mut bytes = Vec::with_capacity(scanned_bytes);
    for chunk in chunks.iter().rev() {
        bytes.extend_from_slice(chunk);
    }
    let search_end = bytes.len() - usize::from(trailing_newline);
    let mut seen = 0usize;
    let mut start = 0usize;
    for index in (0..search_end).rev() {
        if bytes[index] == b'\n' {
            seen += 1;
            if seen == line_limit {
                start = index + 1;
                break;
            }
        }
    }
    let selected = &bytes[start..];
    if selected.len() > MAX_TEXT_BYTES as usize {
        return Err(tail_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "selected event tail exceeds the 1 MiB viewing limit",
        ));
    }
    String::from_utf8(selected.to_vec()).map_err(|error| {
        tail_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("events.jsonl is not UTF-8: {error}"),
        )
    })
}

fn tail_error(status: StatusCode, message: impl Into<String>) -> TailError {
    TailError {
        status,
        message: message.into(),
    }
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        Json(serde_json::json!({ "error": message.into() })),
    )
        .into_response()
}
