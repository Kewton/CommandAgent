use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, bail};

use crate::cli::Cli;

pub(crate) const TEMPLATE: &str = r#"# CommandAgent workspace configuration. Customize the example preset before use.

[preset.local]
model = "qwen3.6:27b-coding-nvfp4"
provider = "ollama"
api = "chat_completions"
tool_protocol = "native"
planner_model = "qwen3.6:27b-coding-nvfp4"
planner_provider = "ollama"
planner_think = "false"
classifier_model = "qwen3.6:27b-coding-nvfp4"
classifier_provider = "ollama"
context_budget = 65536
chat_timeout_secs = 600
profile = "generic"
narration = "normal"
footer = "on"
stream = "on"
prompt_layout = "legacy"
plan_preset = "none"
"#;

pub(crate) fn create(cli: &Cli) -> anyhow::Result<PathBuf> {
    let workspace = match cli.cwd.as_deref() {
        Some(path) => path
            .canonicalize()
            .with_context(|| format!("failed to canonicalize workspace {}", path.display()))?,
        None => std::env::current_dir().context("failed to read current directory")?,
    };
    let directory = workspace.join(".commandagent");
    let path = directory.join("config.toml");
    fs::create_dir_all(&directory)
        .with_context(|| format!("failed to create {}", directory.display()))?;

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = match options.open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            bail!(
                "config file {} already exists; refusing to overwrite it",
                path.display()
            )
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to create config file {}", path.display()));
        }
    };
    file.write_all(TEMPLATE.as_bytes())
        .with_context(|| format!("failed to write config file {}", path.display()))?;
    file.flush()
        .with_context(|| format!("failed to flush config file {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn creates_valid_template_once() {
        let directory = tempfile::tempdir().unwrap();
        let cli = Cli::parse_from([
            "commandagent",
            "--cwd",
            directory.path().to_str().unwrap(),
            "--init-config",
        ]);

        let path = create(&cli).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        let parsed = contents.parse::<toml::Value>().unwrap();
        assert!(parsed["preset"]["local"].is_table());

        let error = create(&cli).unwrap_err().to_string();
        assert!(error.contains("refusing to overwrite"), "{error}");
        assert_eq!(fs::read_to_string(path).unwrap(), contents);
    }
}
