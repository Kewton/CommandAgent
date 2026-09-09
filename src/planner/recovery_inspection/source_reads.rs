//! Line ranges sized for the actual compact conversation tool result.
use serde::{Deserialize, Serialize};
use std::path::Path;

pub(super) fn supports(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("js" | "jsx" | "ts" | "tsx")
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SourceRead {
    pub(crate) path: String,
    pub(crate) start_line: usize,
    pub(crate) end_line: usize,
}

pub(super) fn ranges(path: &str, content: &str) -> Vec<SourceRead> {
    let lines: Vec<_> = content.lines().collect();
    let mut out = Vec::new();
    let mut start = 0;
    let mut characters = 0;
    // ConversationMessage::tool_result retains 500 characters, including the
    // displayed path. Leave room for formatting and never rely on head/tail
    // summarization. Export boundaries keep small interfaces together; larger
    // definitions are covered by contiguous, compact windows.
    let budget = 450usize.saturating_sub(path.chars().count()).max(1);
    for (index, line) in lines.iter().enumerate() {
        let size = line.chars().count() + 1;
        if index > start
            && (characters + size > budget
                || index - start >= 100
                || line.trim_start().starts_with("export "))
        {
            out.push(SourceRead {
                path: path.into(),
                start_line: start + 1,
                end_line: index,
            });
            start = index;
            characters = 0;
        }
        characters += size;
    }
    out.push(SourceRead {
        path: path.into(),
        start_line: start + 1,
        end_line: lines.len().max(1),
    });
    out
}

pub(super) fn render(path: &str, content: &str, reads: &[SourceRead]) -> String {
    let definitions = content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let rest = line.trim_start().strip_prefix("export ")?;
            let rest = rest.strip_prefix("async ").unwrap_or(rest);
            let name = rest
                .split_whitespace()
                .nth(1)?
                .split(['(', '<', ':', '='])
                .next()?;
            Some(format!("{name}@{}", index + 1))
        })
        .collect::<Vec<_>>()
        .join(",");
    let windows = reads
        .iter()
        .map(|read| format!("{}-{}", read.start_line, read.end_line))
        .collect::<Vec<_>>()
        .join(",");
    format!("- {path}: ranges={windows}; definitions={definitions}\n")
}
