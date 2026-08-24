use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub(crate) const COMPACT_HEAD_LINES: usize = 20;
const COMPACT_HEAD_BYTES: usize = 4_000;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ReadRequest {
    path: PathBuf,
    start_line: Option<usize>,
    end_line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileFingerprint([u8; 32]);

#[derive(Debug, Clone)]
struct CachedRead {
    fingerprint: FileFingerprint,
    display_path: String,
    head: String,
    repeat_count: usize,
}

#[derive(Debug)]
pub(crate) struct PendingRead {
    request: ReadRequest,
    fingerprint: Option<FileFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnchangedRead {
    pub(crate) response: String,
    pub(crate) path: String,
    pub(crate) repeat_count: usize,
    pub(crate) identical_consecutive: bool,
    pub(crate) completion_candidate: bool,
}

#[derive(Debug)]
pub(crate) enum ReadDecision {
    Full(PendingRead),
    Unchanged(UnchangedRead),
}

#[derive(Debug, Default)]
pub(crate) struct RepeatedReadCache {
    entries: BTreeMap<ReadRequest, CachedRead>,
    last_read: Option<ReadRequest>,
    written_paths: BTreeSet<PathBuf>,
}

impl RepeatedReadCache {
    pub(crate) fn begin_read(
        &mut self,
        path: &Path,
        start_line: Option<usize>,
        end_line: Option<usize>,
    ) -> ReadDecision {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let request = ReadRequest {
            path: path.clone(),
            start_line,
            end_line,
        };
        let identical_consecutive = self.last_read.as_ref() == Some(&request);
        self.last_read = Some(request.clone());

        let fingerprint = file_fingerprint(&path).ok().flatten();
        let Some(fingerprint) = fingerprint else {
            return ReadDecision::Full(PendingRead {
                request,
                fingerprint: None,
            });
        };
        let Some(cached) = self.entries.get_mut(&request) else {
            return ReadDecision::Full(PendingRead {
                request,
                fingerprint: Some(fingerprint),
            });
        };
        if cached.fingerprint != fingerprint {
            return ReadDecision::Full(PendingRead {
                request,
                fingerprint: Some(fingerprint),
            });
        }

        cached.repeat_count += 1;
        let completion_candidate = identical_consecutive && self.written_paths.contains(&path);
        ReadDecision::Unchanged(UnchangedRead {
            response: compact_response(cached, completion_candidate),
            path: cached.display_path.clone(),
            repeat_count: cached.repeat_count,
            identical_consecutive,
            completion_candidate,
        })
    }

    pub(crate) fn record_full_read(&mut self, pending: PendingRead, output: &str) {
        let Some(fingerprint) = pending.fingerprint else {
            return;
        };
        let mut lines = output.lines();
        let display_path = lines.next().unwrap_or_default().to_string();
        let head = compact_head(lines);
        self.entries.insert(
            pending.request,
            CachedRead {
                fingerprint,
                display_path,
                head,
                repeat_count: 0,
            },
        );
    }

    pub(crate) fn note_non_read_call(&mut self) {
        self.last_read = None;
    }

    pub(crate) fn note_successful_write(&mut self, path: &Path) {
        self.written_paths
            .insert(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()));
    }
}

fn file_fingerprint(path: &Path) -> std::io::Result<Option<FileFingerprint>> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(path)?;
    Ok(Some(FileFingerprint(Sha256::digest(bytes).into())))
}

fn compact_head<'a>(lines: impl Iterator<Item = &'a str>) -> String {
    let head = lines
        .take(COMPACT_HEAD_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    crate::util::excerpt_with_newline_marker(
        &head,
        COMPACT_HEAD_BYTES,
        "[commandagent: cached head truncated]",
    )
}

fn compact_response(cached: &CachedRead, completion_candidate: bool) -> String {
    let completion = if completion_candidate {
        " This identical consecutive Read follows a successful write to this path and is a completion candidate; finish now if the requested work is complete."
    } else {
        " Avoid reading it again unless the file changes."
    };
    format!(
        "{}\n[commandagent: unchanged since the previous matching Read; returning at most the first {COMPACT_HEAD_LINES} cached lines.{completion}]\n{}",
        cached.display_path, cached.head
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_read(cache: &mut RepeatedReadCache, path: &Path, output: &str) {
        let ReadDecision::Full(pending) = cache.begin_read(path, None, None) else {
            panic!("expected full read");
        };
        cache.record_full_read(pending, output);
    }

    #[test]
    fn unchanged_identical_read_is_compact_and_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.txt");
        let content = (1..=25)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, &content).unwrap();
        let output = format!("sample.txt\n{content}");
        let mut cache = RepeatedReadCache::default();
        full_read(&mut cache, &path, &output);

        let ReadDecision::Unchanged(unchanged) = cache.begin_read(&path, None, None) else {
            panic!("expected compact unchanged read");
        };

        assert!(unchanged.identical_consecutive);
        assert_eq!(unchanged.repeat_count, 1);
        assert!(unchanged.response.contains("unchanged since"));
        assert!(unchanged.response.contains("line 20"));
        assert!(!unchanged.response.contains("line 21"));
        assert!(!unchanged.completion_candidate);
    }

    #[test]
    fn changed_file_returns_full_decision_and_refreshes_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, "before").unwrap();
        let mut cache = RepeatedReadCache::default();
        full_read(&mut cache, &path, "sample.txt\nbefore");

        std::fs::write(&path, "after").unwrap();
        let ReadDecision::Full(pending) = cache.begin_read(&path, None, None) else {
            panic!("changed file must receive a full read");
        };
        cache.record_full_read(pending, "sample.txt\nafter");

        let ReadDecision::Unchanged(unchanged) = cache.begin_read(&path, None, None) else {
            panic!("refreshed read should compact");
        };
        assert!(unchanged.response.contains("after"));
        assert!(!unchanged.response.contains("before"));
    }

    #[test]
    fn only_consecutive_reads_of_a_written_path_are_completion_candidates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.txt");
        std::fs::write(&path, "content").unwrap();
        let mut cache = RepeatedReadCache::default();
        cache.note_successful_write(&path);
        full_read(&mut cache, &path, "sample.txt\ncontent");

        cache.note_non_read_call();
        let ReadDecision::Unchanged(non_consecutive) = cache.begin_read(&path, None, None) else {
            panic!("expected unchanged read");
        };
        assert!(!non_consecutive.identical_consecutive);
        assert!(!non_consecutive.completion_candidate);

        let ReadDecision::Unchanged(consecutive) = cache.begin_read(&path, None, None) else {
            panic!("expected consecutive unchanged read");
        };
        assert!(consecutive.identical_consecutive);
        assert!(consecutive.completion_candidate);
        assert!(consecutive.response.contains("finish now"));
    }
}
