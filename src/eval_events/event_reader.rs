use std::path::Path;

use anyhow::Context;
use serde_json::Value;

/// Why one physical line of an events file could not be read as an event.
///
/// The variant deliberately carries no line content: a malformed line may hold
/// a protected value, and a diagnostic is only required to name the file and
/// the line.
#[derive(Debug)]
pub(crate) enum LineFault {
    NotUtf8,
    NotJson(serde_json::Error),
}

impl std::fmt::Display for LineFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotUtf8 => formatter.write_str("line is not valid UTF-8"),
            Self::NotJson(error) => write!(formatter, "line is not valid JSON: {error}"),
        }
    }
}

/// Split a whole events file into physical lines, each addressed by its
/// 1-based number.
///
/// The file is never decoded as one UTF-8 string first: a line cut inside a
/// multi-byte character must not hide the whole lines around it. A single
/// trailing LF terminates the last line instead of opening an empty one, and a
/// CR before that LF is not part of the line (the same shape `str::lines`
/// yields for valid UTF-8).
fn physical_lines(bytes: &[u8]) -> Vec<(usize, &[u8])> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    body.split(|byte| *byte == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
        .enumerate()
        .map(|(index, line)| (index + 1, line))
        .collect()
}

/// Validate one physical line as UTF-8 and then as JSON.
fn parse_line(line: &[u8]) -> Result<Value, LineFault> {
    let text = std::str::from_utf8(line).map_err(|_| LineFault::NotUtf8)?;
    serde_json::from_str(text).map_err(LineFault::NotJson)
}

/// Read every line strictly, failing on the first line that is not one JSON
/// value, and name the file and that line in the error.
pub(crate) fn read_strict(path: &Path) -> anyhow::Result<Vec<Value>> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read run events {}", path.display()))?;
    let mut events = Vec::new();
    for (number, line) in physical_lines(&bytes) {
        let event = parse_line(line).map_err(|fault| {
            anyhow::anyhow!(
                "failed to parse run event {}:{}: {fault}",
                path.display(),
                number
            )
        })?;
        events.push(event);
    }
    Ok(events)
}

/// Read every line that is one JSON value and skip the rest. A line cut inside
/// a multi-byte character is skipped on its own; the lines before and after it
/// are still returned, in file order.
pub(crate) fn read_lenient(path: &Path) -> Vec<Value> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    physical_lines(&bytes)
        .into_iter()
        .filter_map(|(_, line)| parse_line(line).ok())
        .collect()
}

/// [`read_lenient`] for an optional events path.
pub(crate) fn read_lenient_opt(path: Option<&Path>) -> Vec<Value> {
    path.map(read_lenient).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(&path, bytes).unwrap();
        (dir, path)
    }

    #[test]
    fn a_cut_utf8_line_does_not_hide_the_lines_around_it() {
        let bytes =
            b"{\"id\":\"before\"}\n{\"id\":\"cut\",\"text\":\"\xE8\xA9\"}\n{\"id\":\"after\"}\n";
        let (_dir, path) = write(bytes);

        let lenient = read_lenient(&path);
        let ids = lenient
            .iter()
            .map(|event| event["id"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["before", "after"]);

        let error = read_strict(&path).unwrap_err().to_string();
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert!(error.contains(":2:"), "{error}");
        assert!(error.contains("not valid UTF-8"), "{error}");
    }

    #[test]
    fn a_malformed_json_line_is_named_by_file_and_line() {
        let (_dir, path) = write(b"{\"id\":\"before\"}\nnot-json\n{\"id\":\"after\"}\n");

        assert_eq!(
            read_lenient(&path)
                .iter()
                .map(|event| event["id"].as_str().unwrap().to_string())
                .collect::<Vec<_>>(),
            ["before", "after"]
        );
        let error = read_strict(&path).unwrap_err().to_string();
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert!(error.contains(":2:"), "{error}");
    }

    #[test]
    fn a_last_line_without_lf_is_still_a_line() {
        let (_dir, path) = write(b"{\"id\":\"a\"}\n{\"id\":\"b\"}");

        assert_eq!(read_lenient(&path).len(), 2);
        assert_eq!(read_strict(&path).unwrap().len(), 2);
    }

    #[test]
    fn an_empty_file_has_no_lines() {
        let (_dir, path) = write(b"");
        assert!(read_lenient(&path).is_empty());
        assert!(read_strict(&path).unwrap().is_empty());
    }
}
