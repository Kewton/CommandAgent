use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;

pub fn append(state_root: &Path, gate: &str, body: &str) -> anyhow::Result<PathBuf> {
    let path = state_root.join("boundary-transcript.md");
    std::fs::create_dir_all(state_root)
        .with_context(|| format!("create transcript root {}", state_root.display()))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open boundary transcript {}", path.display()))?;
    writeln!(file, "\n## {gate}\n\n{body}\n")?;
    file.sync_all()?;
    Ok(path)
}
