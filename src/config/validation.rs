//! Startup validation of the layered TOML config files for
//! [`Config`](super::Config).
//!
//! Every existing file among the four `config.toml` layers is parsed on each
//! startup, whether or not a preset or CLI override is in play. An unknown
//! top-level key, invalid TOML or an unreadable file is an error; a broken
//! higher-precedence file is never skipped in favour of a lower layer or the
//! default. Diagnostics name the file, line and key only, never a value.

use std::path::Path;

use anyhow::bail;

use super::{SUPPORTED_TOP_LEVEL_KEYS, config_paths, strip_toml_comment};

/// Check every existing config layer for unreadable files, invalid TOML and
/// unknown top-level keys, failing on the first problem. Values and preset
/// bodies are not examined here: `${ENV}` expansion and preset validation stay
/// with the selected preset, so an unselected preset never blocks startup.
pub(super) fn validate_config_files(root: &Path) -> anyhow::Result<()> {
    for path in config_paths(root) {
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => bail!("failed to read {}: {err}", path.display()),
        };
        if let Err(error) = text.parse::<toml::Table>() {
            bail!("{}", invalid_toml_location(&path, &text, &error));
        }
        let mut top_level = true;
        for (index, raw_line) in text.lines().enumerate() {
            let line = strip_toml_comment(raw_line).trim();
            if line.starts_with('[') {
                top_level = false;
                continue;
            }
            if !top_level {
                continue;
            }
            let Some((key, _)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if !SUPPORTED_TOP_LEVEL_KEYS.contains(&key) {
                bail!(
                    "{}:{} unknown config key '{key}'",
                    path.display(),
                    index + 1
                );
            }
        }
    }
    Ok(())
}

/// One-line location of a TOML syntax error: the file and the 1-based line,
/// without the parser's source excerpt, which would echo the offending value.
pub(super) fn invalid_toml_location(path: &Path, text: &str, error: &toml::de::Error) -> String {
    let line = error.span().map(|span| {
        let end = span.start.min(text.len());
        text.as_bytes()[..end]
            .iter()
            .filter(|b| **b == b'\n')
            .count()
            + 1
    });
    match line {
        Some(line) => format!("{}:{line} contains invalid TOML syntax", path.display()),
        None => format!("{} contains invalid TOML syntax", path.display()),
    }
}
