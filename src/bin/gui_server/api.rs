use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path as FilePath, PathBuf};
use std::time::UNIX_EPOCH;

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use commandagent::tui::boundary_shell::band_catalog::{BAND_VALUES, BandValue};
use commandagent::tui::boundary_shell::family_catalog::TaskFamilyId;
use serde::{Deserialize, Serialize};

use super::AppState;
use super::error_response::GuiError;

pub(super) const MAX_TEXT_BYTES: u64 = 1_048_576;
pub(super) const MAX_LIST_ENTRIES: usize = 256;

#[derive(Debug, Serialize)]
pub struct RunSummary {
    id: String,
    modified_epoch_seconds: u64,
    report_path: Option<String>,
    status: String,
    status_text: String,
    state: RunState,
}

#[derive(Debug, Serialize)]
pub struct RunIndex {
    runs: Vec<RunSummary>,
    total: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RunState {
    Pass,
    Fail,
    Pending,
    Unknown,
}

#[derive(Debug, Eq, PartialEq)]
struct ExtractedStatus {
    state: RunState,
    text: String,
}

#[derive(Debug, Serialize)]
pub struct RunDetail {
    id: String,
    acceptance_path: Option<String>,
    acceptance: String,
    evidence: Vec<DocumentSummary>,
}

#[derive(Debug, Serialize)]
pub struct Document {
    id: String,
    path: String,
    content: String,
}

impl Document {
    pub(super) fn redact_execution_root(&mut self, execution_root: &FilePath) {
        self.content = super::public_projection::text(&self.content, execution_root);
    }
}

#[derive(Debug, Serialize)]
pub struct BandMean {
    profile: &'static str,
    intent: &'static str,
    family: &'static str,
    duration_n: usize,
    average_duration_seconds: Option<f64>,
    source: &'static str,
}

#[derive(Debug, Serialize)]
pub struct DocumentSummary {
    id: String,
    path: String,
    size_bytes: u64,
}

#[derive(Debug, Deserialize)]
pub struct EvidenceQuery {
    path: String,
}

pub type ApiError = GuiError;

pub async fn runs(State(state): State<AppState>) -> Result<Json<RunIndex>, ApiError> {
    let root = state.repository_root.join("workspace/management/runs");
    let mut entries = directory_entries(&root).await?;
    entries.retain(|path| path.is_dir());
    let total = entries.len();
    entries.sort_by(|left, right| right.file_name().cmp(&left.file_name()));
    entries.truncate(100);

    let mut summaries = Vec::with_capacity(entries.len());
    for path in entries {
        let id = file_name(&path)?;
        let modified_epoch_seconds = modified_epoch_seconds(&path).await;
        let report = preferred_report(&path).await;
        let (report_path, extracted_status) = match report {
            Some(report) => {
                let content = read_text(&report).await.unwrap_or_default();
                let relative = relative_string(&state.repository_root, &report)?;
                (Some(relative), extract_status(&content))
            }
            None => (None, status_value("not recorded")),
        };
        let status_text = extracted_status.text;
        summaries.push(RunSummary {
            id,
            modified_epoch_seconds,
            report_path,
            status: status_text.clone(),
            status_text,
            state: extracted_status.state,
        });
    }
    summaries.sort_by(|left, right| {
        right
            .modified_epoch_seconds
            .cmp(&left.modified_epoch_seconds)
            .then_with(|| right.id.cmp(&left.id))
    });
    Ok(Json(RunIndex {
        runs: summaries,
        total,
    }))
}

pub async fn run_detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<RunDetail>, ApiError> {
    require_component(&id)?;
    let runs_root = state.repository_root.join("workspace/management/runs");
    let run_root = checked_existing_directory(&runs_root, FilePath::new(&id)).await?;
    let documents = collect_documents(&run_root, 4).await?;
    let acceptance_file = choose_acceptance(&documents);
    let (acceptance_path, acceptance) = match acceptance_file {
        Some(path) => (
            Some(relative_string(&run_root, path)?),
            read_text(path).await?,
        ),
        None => (None, "No acceptance sheet or report was found.".to_string()),
    };
    let evidence = documents
        .iter()
        .filter(|path| acceptance_file != Some(*path))
        .filter_map(|path| document_summary(&run_root, path))
        .take(MAX_LIST_ENTRIES)
        .collect();
    Ok(Json(RunDetail {
        id,
        acceptance_path,
        acceptance,
        evidence,
    }))
}

pub async fn run_evidence(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<EvidenceQuery>,
) -> Result<Json<Document>, ApiError> {
    require_component(&id)?;
    let run_root = state
        .repository_root
        .join("workspace/management/runs")
        .join(&id);
    let path = checked_existing_path(&run_root, FilePath::new(&query.path)).await?;
    Ok(Json(document(&run_root, &path).await?))
}

pub async fn bands(State(state): State<AppState>) -> Result<Json<Vec<Document>>, ApiError> {
    let root = state.repository_root.join("workspace/management/runs");
    documents_matching(&root, 1, |path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("band_summary") && name.ends_with(".md"))
    })
    .await
}

pub async fn band_means(State(state): State<AppState>) -> Result<Json<Vec<BandMean>>, ApiError> {
    let mut sources = BTreeMap::new();
    for band in BAND_VALUES {
        if !sources.contains_key(band.source) {
            let path = state.repository_root.join(band.source);
            let text = match tokio::fs::try_exists(&path).await {
                Ok(false) => None,
                Ok(true) => {
                    let path = checked_existing_path_without_symlinks(
                        &state.repository_root,
                        FilePath::new(band.source),
                    )
                    .await?;
                    Some(read_text(&path).await?)
                }
                Err(error) => {
                    return Err(internal(format!("read {}: {error}", path.display())));
                }
            };
            sources.insert(band.source, text);
        }
    }
    let means = BAND_VALUES
        .iter()
        .map(|band| {
            let durations = sources
                .get(band.source)
                .and_then(Option::as_deref)
                .map_or_else(Vec::new, |text| band_durations(text, band));
            BandMean {
                profile: band.profile,
                intent: band.intent.as_str(),
                family: band.family.as_str(),
                duration_n: durations.len(),
                average_duration_seconds: mean(&durations),
                source: band.source,
            }
        })
        .collect();
    Ok(Json(means))
}

pub async fn maps(State(state): State<AppState>) -> Result<Json<Vec<DocumentSummary>>, ApiError> {
    let root = state.repository_root.join("workspace/management/runs");
    let documents = collect_documents(&root, 1).await?;
    Ok(Json(
        documents
            .iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("score_time_map."))
            })
            .filter_map(|path| document_summary(&root, path))
            .collect(),
    ))
}

pub async fn score_time_map(State(state): State<AppState>) -> Result<Response, ApiError> {
    let path = state
        .repository_root
        .join("workspace/management/runs/score_time_map.svg");
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|error| internal(format!("read {}: {error}", path.display())))?;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/svg+xml; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from(bytes),
    )
        .into_response())
}

pub async fn packs(
    State(state): State<AppState>,
) -> Result<Json<Vec<super::pack_catalog::PackSummary>>, ApiError> {
    super::pack_catalog::list(state.repository_root, state.extension_root)
        .await
        .map(Json)
        .map_err(internal)
}

pub async fn contracts(State(state): State<AppState>) -> Result<Json<Vec<Document>>, ApiError> {
    let root = state.repository_root.join("docs");
    documents_matching(&root, 3, |path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains("contract") && name.ends_with(".md"))
    })
    .await
}

pub async fn suites(State(state): State<AppState>) -> Result<Json<Vec<Document>>, ApiError> {
    let root = state.repository_root.join("workspace/management/bench");
    documents_matching(&root, 2, |path| {
        path.extension().and_then(|ext| ext.to_str()) == Some("toml")
    })
    .await
}

pub async fn reports(
    State(state): State<AppState>,
) -> Result<Json<Vec<DocumentSummary>>, ApiError> {
    let root = state.repository_root.join("workspace/management/runs");
    let documents = collect_documents(&root, 2).await?;
    let mut reports = documents
        .iter()
        .filter(|path| is_measurement_report(path))
        .filter_map(|path| document_summary(&root, path))
        .collect::<Vec<_>>();
    reports.sort_by(|left, right| right.path.cmp(&left.path));
    reports.truncate(MAX_LIST_ENTRIES);
    Ok(Json(reports))
}

pub async fn report_content(
    State(state): State<AppState>,
    Query(query): Query<EvidenceQuery>,
) -> Result<Json<Document>, ApiError> {
    let root = state.repository_root.join("workspace/management/runs");
    let path = checked_existing_path(&root, FilePath::new(&query.path)).await?;
    if !is_measurement_report(&path) {
        return Err(not_found("requested document is not a measurement report"));
    }
    Ok(Json(document(&root, &path).await?))
}

async fn documents_matching(
    root: &FilePath,
    max_depth: usize,
    predicate: impl Fn(&FilePath) -> bool,
) -> Result<Json<Vec<Document>>, ApiError> {
    let paths = collect_documents(root, max_depth).await?;
    let mut documents = Vec::new();
    for path in paths.into_iter().filter(|path| predicate(path)) {
        documents.push(document(root, &path).await?);
    }
    documents.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(Json(documents))
}

pub(super) async fn document(root: &FilePath, path: &FilePath) -> Result<Document, ApiError> {
    Ok(Document {
        id: file_name(path)?,
        path: relative_string(root, path)?,
        content: read_text(path).await?,
    })
}

pub(super) fn document_summary(root: &FilePath, path: &FilePath) -> Option<DocumentSummary> {
    let size_bytes = path.metadata().ok()?.len();
    Some(DocumentSummary {
        id: path.file_name()?.to_str()?.to_string(),
        path: relative_string(root, path).ok()?,
        size_bytes,
    })
}

pub(super) async fn collect_documents(
    root: &FilePath,
    max_depth: usize,
) -> Result<Vec<PathBuf>, ApiError> {
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut documents = Vec::new();
    let skipped_directories = BTreeSet::from([".git", ".next", "node_modules", "target"]);
    while let Some((directory, depth)) = pending.pop() {
        let mut entries = match tokio::fs::read_dir(&directory).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(internal(format!("read {}: {error}", directory.display()))),
        };
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| internal(format!("read {}: {error}", directory.display())))?
        {
            let path = entry.path();
            let file_type = entry
                .file_type()
                .await
                .map_err(|error| internal(format!("inspect {}: {error}", path.display())))?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() && depth < max_depth {
                if !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| skipped_directories.contains(name))
                {
                    pending.push((path, depth + 1));
                }
            } else if file_type.is_file() && is_text_document(&path) {
                documents.push(path);
                if documents.len() >= MAX_LIST_ENTRIES * 8 {
                    break;
                }
            }
        }
    }
    documents.sort();
    Ok(documents)
}

async fn directory_entries(root: &FilePath) -> Result<Vec<PathBuf>, ApiError> {
    let mut entries = tokio::fs::read_dir(root)
        .await
        .map_err(|error| internal(format!("read {}: {error}", root.display())))?;
    let mut paths = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| internal(format!("read {}: {error}", root.display())))?
    {
        if !entry
            .file_type()
            .await
            .map_err(|error| internal(format!("inspect {}: {error}", entry.path().display())))?
            .is_symlink()
        {
            paths.push(entry.path());
        }
    }
    Ok(paths)
}

async fn preferred_report(run_root: &FilePath) -> Option<PathBuf> {
    for name in [
        "acceptance-sheet.md",
        "uat-report.md",
        "report.md",
        "final-report.md",
        "summary.md",
    ] {
        let path = run_root.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn choose_acceptance(documents: &[PathBuf]) -> Option<&PathBuf> {
    let priorities = [
        "acceptance-sheet.md",
        "uat-report.md",
        "report.md",
        "final-report.md",
        "summary.md",
    ];
    priorities.iter().find_map(|name| {
        documents
            .iter()
            .find(|path| path.file_name().and_then(|file_name| file_name.to_str()) == Some(*name))
    })
}

fn extract_status(content: &str) -> ExtractedStatus {
    content
        .lines()
        .find_map(|line| {
            let normalized = line.trim().trim_start_matches(['-', '#', ' ']).trim();
            ["Task status:", "Status:", "Overall:"]
                .iter()
                .find_map(|prefix| {
                    normalized
                        .strip_prefix(prefix)
                        .map(status_value)
                        .filter(|status| !status.text.is_empty())
                })
        })
        .unwrap_or_else(recorded_status)
}

fn recorded_status() -> ExtractedStatus {
    ExtractedStatus {
        state: RunState::Pending,
        text: "recorded".to_string(),
    }
}

fn status_value(value: &str) -> ExtractedStatus {
    let text = value
        .trim()
        .chars()
        .filter(|character| !matches!(character, '*' | '`'))
        .collect::<String>()
        .trim()
        .to_string();
    let words = text
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<BTreeSet<_>>();
    let state = if [
        "abort", "aborted", "block", "blocked", "fail", "failed", "failure",
    ]
    .iter()
    .any(|word| words.contains(*word))
    {
        RunState::Fail
    } else if [
        "complete",
        "completed",
        "full",
        "green",
        "merge",
        "merged",
        "pass",
        "passed",
    ]
    .iter()
    .any(|word| words.contains(*word))
    {
        RunState::Pass
    } else if ["open", "pending", "progress", "running"]
        .iter()
        .any(|word| words.contains(*word))
    {
        RunState::Pending
    } else {
        RunState::Unknown
    };
    ExtractedStatus { state, text }
}

fn is_measurement_report(path: &FilePath) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.contains("report")
        || name.contains("measurement")
        || name == "score_time_map.md"
        || name == "study-summary.json"
}

fn band_durations(text: &str, band: &BandValue) -> Vec<f64> {
    let mut durations = Vec::new();
    let mut headers: Option<Vec<String>> = None;
    for line in text.lines() {
        if !line.trim_start().starts_with('|') {
            headers = None;
            continue;
        }
        let cells = markdown_cells(line);
        if cells.iter().all(|cell| {
            let trimmed = cell.trim_matches(['-', ':', ' ']);
            trimmed.is_empty()
        }) {
            continue;
        }
        let has_family = cells
            .iter()
            .any(|cell| matches!(cell.as_str(), "Family" | "Scenario"));
        let has_duration = cells
            .iter()
            .any(|cell| matches!(cell.as_str(), "Seconds" | "Duration"));
        let has_band_scope = cells
            .iter()
            .any(|cell| matches!(cell.as_str(), "Band status" | "Window"));
        if has_family
            && has_duration
            && (has_band_scope || cells.iter().any(|cell| cell == "Seconds"))
        {
            headers = Some(cells);
            continue;
        }
        let Some(header) = headers.as_ref() else {
            continue;
        };
        if cells.len() != header.len() {
            continue;
        }
        let field = |names: &[&str]| {
            header
                .iter()
                .position(|value| names.contains(&value.as_str()))
                .and_then(|index| cells.get(index))
                .map(String::as_str)
        };
        if field(&["Family", "Scenario"])
            .is_none_or(|family| TaskFamilyId::parse(family) != band.family)
        {
            continue;
        }
        if let Some(scope) = field(&["Band status", "Window"])
            && !band_scope_matches(scope, band.arm)
        {
            continue;
        }
        if let Some(duration) = field(&["Seconds", "Duration"]).and_then(parse_duration_seconds) {
            durations.push(duration);
        }
    }
    durations
}

fn markdown_cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn band_scope_matches(value: &str, arm: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    let arm = arm.trim().to_ascii_lowercase();
    value.contains(&arm)
        || arm
            .strip_prefix("window ")
            .is_some_and(|window| value == window)
}

fn parse_duration_seconds(value: &str) -> Option<f64> {
    let value = value.trim().trim_matches('`').trim();
    if let Ok(seconds) = value.parse::<f64>() {
        return seconds.is_finite().then_some(seconds);
    }
    if let Some(seconds) = value.strip_suffix('s') {
        if let Ok(seconds) = seconds.parse::<f64>() {
            return seconds.is_finite().then_some(seconds);
        }
        if let Some((minutes, seconds)) = seconds.split_once('m') {
            let minutes = minutes.parse::<f64>().ok()?;
            let seconds = seconds.parse::<f64>().ok()?;
            let total = minutes * 60.0 + seconds;
            return total.is_finite().then_some(total);
        }
    }
    None
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn is_text_document(path: &FilePath) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("json" | "jsonl" | "md" | "sha256" | "svg" | "toml" | "txt" | "yaml" | "yml")
    )
}

async fn read_text(path: &FilePath) -> Result<String, ApiError> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|error| not_found(format!("read {}: {error}", path.display())))?;
    if metadata.len() > MAX_TEXT_BYTES {
        return Err(GuiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "resource_too_large",
            format!("{} exceeds the 1 MiB viewing limit", path.display()),
        ));
    }
    tokio::fs::read_to_string(path)
        .await
        .map_err(|error| internal(format!("read {}: {error}", path.display())))
}

async fn checked_existing_path(root: &FilePath, relative: &FilePath) -> Result<PathBuf, ApiError> {
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(not_found("invalid relative path"));
    }
    let canonical_root = tokio::fs::canonicalize(root)
        .await
        .map_err(|error| not_found(format!("read {}: {error}", root.display())))?;
    let candidate = root.join(relative);
    let canonical_candidate = tokio::fs::canonicalize(&candidate)
        .await
        .map_err(|error| not_found(format!("document not found: {error}")))?;
    if !canonical_candidate.starts_with(&canonical_root)
        || !canonical_candidate.is_file()
        || !is_text_document(&canonical_candidate)
    {
        return Err(not_found("document is outside the readable inventory"));
    }
    Ok(candidate)
}

pub(super) async fn checked_existing_path_without_symlinks(
    root: &FilePath,
    relative: &FilePath,
) -> Result<PathBuf, ApiError> {
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(not_found("invalid relative path"));
    }
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(not_found("invalid relative path"));
        };
        current.push(component);
        let metadata = tokio::fs::symlink_metadata(&current)
            .await
            .map_err(|error| not_found(format!("document not found: {error}")))?;
        if metadata.file_type().is_symlink() {
            return Err(not_found("document symlinks are not readable"));
        }
    }
    checked_existing_path(root, relative).await
}

pub(super) async fn checked_existing_directory(
    root: &FilePath,
    relative: &FilePath,
) -> Result<PathBuf, ApiError> {
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(not_found("invalid relative path"));
    }
    let canonical_root = tokio::fs::canonicalize(root)
        .await
        .map_err(|error| not_found(format!("read {}: {error}", root.display())))?;
    let candidate = root.join(relative);
    let canonical_candidate = tokio::fs::canonicalize(&candidate)
        .await
        .map_err(|error| not_found(format!("run not found: {error}")))?;
    if !canonical_candidate.starts_with(canonical_root) || !canonical_candidate.is_dir() {
        return Err(not_found("run is outside the readable inventory"));
    }
    Ok(candidate)
}

async fn modified_epoch_seconds(path: &FilePath) -> u64 {
    tokio::fs::metadata(path)
        .await
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_secs())
}

fn require_component(value: &str) -> Result<(), ApiError> {
    if value.is_empty()
        || FilePath::new(value)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        Err(not_found("invalid run id"))
    } else {
        Ok(())
    }
}

fn relative_string(root: &FilePath, path: &FilePath) -> Result<String, ApiError> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| internal(format!("{} is outside {}", path.display(), root.display())))
}

fn file_name(path: &FilePath) -> Result<String, ApiError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| internal(format!("{} has no UTF-8 file name", path.display())))
}

fn not_found(message: impl Into<String>) -> ApiError {
    GuiError::new(StatusCode::NOT_FOUND, "resource_not_found", message)
}

fn internal(message: impl Into<String>) -> ApiError {
    GuiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "repository_read_failed",
        message,
    )
}

#[cfg(test)]
mod tests {
    use commandagent::planner::adjudication::contract::IntentId;
    use commandagent::tui::boundary_shell::band_catalog::BandValue;
    use commandagent::tui::boundary_shell::family_catalog::TaskFamilyId;

    use super::{ExtractedStatus, RunState, band_durations, extract_status};

    #[test]
    fn extract_status_normalizes_markdown_and_enumerates_pass() {
        assert_eq!(
            extract_status("# Acceptance\n\nStatus: **FULL 3/3 (2026-08-03)**\n"),
            ExtractedStatus {
                state: RunState::Pass,
                text: "FULL 3/3 (2026-08-03)".to_string(),
            }
        );
    }

    #[test]
    fn extract_status_removes_code_markers_and_distinguishes_available_evidence() {
        assert_eq!(
            extract_status("- Overall: `NOT RECORDED`\n"),
            ExtractedStatus {
                state: RunState::Unknown,
                text: "NOT RECORDED".to_string(),
            }
        );
        assert_eq!(
            extract_status("# Report without a status field\n"),
            ExtractedStatus {
                state: RunState::Pending,
                text: "recorded".to_string(),
            }
        );
    }

    #[test]
    fn extract_status_classifies_exact_failure_and_pending_words() {
        assert_eq!(
            extract_status("Status: blocked pending owner action\n"),
            ExtractedStatus {
                state: RunState::Fail,
                text: "blocked pending owner action".to_string(),
            }
        );
        assert_eq!(
            extract_status("Task status: verification in progress\n"),
            ExtractedStatus {
                state: RunState::Pending,
                text: "verification in progress".to_string(),
            }
        );
        assert_eq!(
            extract_status("Overall: blockade readiness\n"),
            ExtractedStatus {
                state: RunState::Unknown,
                text: "blockade readiness".to_string(),
            }
        );
    }

    #[test]
    fn band_duration_parser_filters_family_and_formal_arm() {
        let band = BandValue {
            profile: "python-cli",
            intent: IntentId::Create,
            family: TaskFamilyId::Stats,
            full: 0,
            denominator: 3,
            display_rate: "0%",
            arm: "formal Window B",
            measurement: "fixture",
            source: "fixture.md",
            full_meaning: "fixture",
        };
        let text = "\
| Family | Band status | Seconds |\n\
| --- | --- | --- |\n\
| stats | formal Window B | 60 |\n\
| stats | excluded | 900 |\n\
| filter | formal Window B | 120 |\n";

        assert_eq!(band_durations(text, &band), vec![60.0]);
    }

    #[test]
    fn band_duration_parser_accepts_duration_encodings_and_window_short_name() {
        let band = BandValue {
            profile: "data",
            intent: IntentId::Create,
            family: TaskFamilyId::Aggregation,
            full: 2,
            denominator: 6,
            display_rate: "33%",
            arm: "Window B",
            measurement: "fixture",
            source: "fixture.md",
            full_meaning: "fixture",
        };
        let text = "\
| Family | Window | Duration |\n\
| --- | --- | --- |\n\
| aggregation | B | 5m02s |\n\
| aggregation | B | 390s |\n\
| aggregation | A | 999s |\n";

        assert_eq!(band_durations(text, &band), vec![302.0, 390.0]);
    }
}
