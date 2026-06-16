use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Ollama,
    Gemini,
    OpenAi,
}

impl Provider {
    pub fn parse(value: &str) -> Result<Self, ConfigError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ollama" => Ok(Self::Ollama),
            "gemini" => Ok(Self::Gemini),
            "openai" => Ok(Self::OpenAi),
            other => Err(ConfigError::InvalidValue {
                key: "provider".to_string(),
                value: other.to_string(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::Gemini => "gemini",
            Self::OpenAi => "openai",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub cwd: PathBuf,
    pub state_dir: PathBuf,
    pub provider: Provider,
    pub planner_provider: Option<Provider>,
    pub model: Option<String>,
    pub planner_model: Option<String>,
    pub context_budget: usize,
    pub max_iterations: u32,
    pub timeout_secs: u64,
    pub retries: u8,
    pub yes: bool,
    pub offline: bool,
    pub resume: Option<String>,
    pub gemini_api_key: Option<String>,
    pub openai_api_key: Option<String>,
}

impl Config {
    pub fn load(cwd: impl Into<PathBuf>) -> Result<Self, ConfigError> {
        Self::load_from(cwd, std::env::args_os(), |key| std::env::var(key).ok())
    }

    pub fn load_from<I, S, F>(
        cwd: impl Into<PathBuf>,
        args: I,
        env_get: F,
    ) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
        F: Fn(&str) -> Option<String>,
    {
        let cwd = cwd.into();
        let mut raw = RawConfig::defaults(&cwd);

        raw.apply_file(&cwd.join(".commandagent/config"))?;
        raw.apply_env(&env_get);
        raw.apply_cli(args)?;

        raw.into_config(cwd, &env_get)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Io { path: PathBuf, message: String },
    InvalidLine { path: PathBuf, line: usize },
    MissingValue { option: String },
    InvalidValue { key: String, value: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(f, "failed to read {}: {}", path.display(), message)
            }
            Self::InvalidLine { path, line } => {
                write!(f, "invalid config line {} in {}", line, path.display())
            }
            Self::MissingValue { option } => write!(f, "missing value for {}", option),
            Self::InvalidValue { key, value } => write!(f, "invalid value for {}: {}", key, value),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug, Clone)]
struct RawConfig {
    values: HashMap<String, String>,
}

impl RawConfig {
    fn defaults(cwd: &Path) -> Self {
        let mut values = HashMap::new();
        values.insert(
            "state_dir".to_string(),
            cwd.join(".commandagent").display().to_string(),
        );
        values.insert("provider".to_string(), "ollama".to_string());
        values.insert("context_budget".to_string(), "65536".to_string());
        values.insert("max_iterations".to_string(), "8".to_string());
        values.insert("timeout_secs".to_string(), "120".to_string());
        values.insert("retries".to_string(), "2".to_string());
        values.insert("yes".to_string(), "false".to_string());
        values.insert("offline".to_string(), "false".to_string());
        Self { values }
    }

    fn apply_file(&mut self, path: &Path) -> Result<(), ConfigError> {
        if !path.exists() {
            return Ok(());
        }

        let contents = fs::read_to_string(path).map_err(|err| ConfigError::Io {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?;

        for (idx, line) in contents.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                return Err(ConfigError::InvalidLine {
                    path: path.to_path_buf(),
                    line: idx + 1,
                });
            };
            self.set(key, strip_quotes(value.trim()));
        }

        Ok(())
    }

    fn apply_env<F>(&mut self, env_get: &F)
    where
        F: Fn(&str) -> Option<String>,
    {
        let mappings = [
            ("COMMANDAGENT_STATE_DIR", "state_dir"),
            ("COMMANDAGENT_PROVIDER", "provider"),
            ("COMMANDAGENT_PLANNER_PROVIDER", "planner_provider"),
            ("COMMANDAGENT_MODEL", "model"),
            ("COMMANDAGENT_PLANNER_MODEL", "planner_model"),
            ("COMMANDAGENT_CONTEXT_BUDGET", "context_budget"),
            ("COMMANDAGENT_MAX_ITERATIONS", "max_iterations"),
            ("COMMANDAGENT_TIMEOUT_SECS", "timeout_secs"),
            ("COMMANDAGENT_RETRIES", "retries"),
            ("COMMANDAGENT_YES", "yes"),
            ("COMMANDAGENT_OFFLINE", "offline"),
            ("COMMANDAGENT_RESUME", "resume"),
        ];

        for (env_key, config_key) in mappings {
            if let Some(value) = env_get(env_key) {
                self.set(config_key, value);
            }
        }
    }

    fn apply_cli<I, S>(&mut self, args: I) -> Result<(), ConfigError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut args = args.into_iter().map(Into::into).skip(1).peekable();

        while let Some(arg) = args.next() {
            let arg = arg.to_string_lossy();

            if arg == "--yes" {
                self.set("yes", "true");
            } else if arg == "--offline" {
                self.set("offline", "true");
            } else if let Some((key, value)) = arg.split_once('=') {
                if let Some(config_key) = cli_key(key) {
                    self.set(config_key, value.to_string());
                }
            } else if let Some(config_key) = cli_key(&arg) {
                let value = args.next().ok_or_else(|| ConfigError::MissingValue {
                    option: arg.to_string(),
                })?;
                self.set(config_key, value.to_string_lossy().to_string());
            }
        }

        Ok(())
    }

    fn into_config<F>(self, cwd: PathBuf, env_get: &F) -> Result<Config, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let get = |key: &str| self.values.get(key).cloned();
        let provider = Provider::parse(&required(&self.values, "provider")?)?;
        let planner_provider = get("planner_provider")
            .map(|value| Provider::parse(&value))
            .transpose()?;

        Ok(Config {
            cwd,
            state_dir: PathBuf::from(required(&self.values, "state_dir")?),
            provider,
            planner_provider,
            model: get("model"),
            planner_model: get("planner_model"),
            context_budget: parse_usize(&self.values, "context_budget")?,
            max_iterations: parse_u32(&self.values, "max_iterations")?,
            timeout_secs: parse_u64(&self.values, "timeout_secs")?,
            retries: parse_u8(&self.values, "retries")?,
            yes: parse_bool(&self.values, "yes")?,
            offline: parse_bool(&self.values, "offline")?,
            resume: get("resume"),
            gemini_api_key: env_get("GEMINI_API_KEY"),
            openai_api_key: env_get("OPENAI_API_KEY"),
        })
    }

    fn set(&mut self, key: &str, value: impl Into<String>) {
        self.values
            .insert(key.trim().to_ascii_lowercase(), value.into());
    }
}

fn cli_key(key: &str) -> Option<&'static str> {
    match key {
        "--state-dir" => Some("state_dir"),
        "--provider" => Some("provider"),
        "--planner-provider" => Some("planner_provider"),
        "--model" => Some("model"),
        "--planner-model" => Some("planner_model"),
        "--context-budget" => Some("context_budget"),
        "--max-iterations" => Some("max_iterations"),
        "--timeout" | "--timeout-secs" => Some("timeout_secs"),
        "--retries" => Some("retries"),
        "--resume" => Some("resume"),
        _ => None,
    }
}

fn strip_quotes(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
        .to_string()
}

fn required(values: &HashMap<String, String>, key: &str) -> Result<String, ConfigError> {
    values
        .get(key)
        .cloned()
        .ok_or_else(|| ConfigError::MissingValue {
            option: key.to_string(),
        })
}

fn parse_bool(values: &HashMap<String, String>, key: &str) -> Result<bool, ConfigError> {
    match required(values, key)?.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        value => Err(ConfigError::InvalidValue {
            key: key.to_string(),
            value: value.to_string(),
        }),
    }
}

fn parse_usize(values: &HashMap<String, String>, key: &str) -> Result<usize, ConfigError> {
    required(values, key)?
        .parse()
        .map_err(|_| ConfigError::InvalidValue {
            key: key.to_string(),
            value: values.get(key).cloned().unwrap_or_default(),
        })
}

fn parse_u32(values: &HashMap<String, String>, key: &str) -> Result<u32, ConfigError> {
    required(values, key)?
        .parse()
        .map_err(|_| ConfigError::InvalidValue {
            key: key.to_string(),
            value: values.get(key).cloned().unwrap_or_default(),
        })
}

fn parse_u64(values: &HashMap<String, String>, key: &str) -> Result<u64, ConfigError> {
    required(values, key)?
        .parse()
        .map_err(|_| ConfigError::InvalidValue {
            key: key.to_string(),
            value: values.get(key).cloned().unwrap_or_default(),
        })
}

fn parse_u8(values: &HashMap<String, String>, key: &str) -> Result<u8, ConfigError> {
    required(values, key)?
        .parse()
        .map_err(|_| ConfigError::InvalidValue {
            key: key.to_string(),
            value: values.get(key).cloned().unwrap_or_default(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn defaults_to_ollama_and_commandagent_state() {
        let cwd = temp_workspace("defaults");
        let config = Config::load_from(&cwd, ["commandagent"], |_| None).unwrap();

        assert_eq!(config.provider, Provider::Ollama);
        assert_eq!(config.state_dir, cwd.join(".commandagent"));
        assert_eq!(config.context_budget, 65536);
        assert!(!config.yes);
    }

    #[test]
    fn precedence_is_cli_then_env_then_file_then_default() {
        let cwd = temp_workspace("precedence");
        write_config(
            &cwd,
            r#"
provider = "ollama"
model = "file-model"
context_budget = 111
yes = false
"#,
        );

        let env = |key: &str| match key {
            "COMMANDAGENT_PROVIDER" => Some("gemini".to_string()),
            "COMMANDAGENT_MODEL" => Some("env-model".to_string()),
            "COMMANDAGENT_CONTEXT_BUDGET" => Some("222".to_string()),
            "GEMINI_API_KEY" => Some("gemini-key".to_string()),
            "OPENAI_API_KEY" => Some("openai-key".to_string()),
            _ => None,
        };

        let config = Config::load_from(
            &cwd,
            [
                "commandagent",
                "--provider",
                "openai",
                "--model=cli-model",
                "--yes",
            ],
            env,
        )
        .unwrap();

        assert_eq!(config.provider, Provider::OpenAi);
        assert_eq!(config.model.as_deref(), Some("cli-model"));
        assert_eq!(config.context_budget, 222);
        assert!(config.yes);
        assert_eq!(config.gemini_api_key.as_deref(), Some("gemini-key"));
        assert_eq!(config.openai_api_key.as_deref(), Some("openai-key"));
    }

    #[test]
    fn planner_provider_and_resume_are_supported() {
        let cwd = temp_workspace("planner");
        let config = Config::load_from(
            &cwd,
            [
                "commandagent",
                "--planner-provider",
                "gemini",
                "--planner-model",
                "gemini-3.5-flash",
                "--resume",
                "session-1",
                "--offline",
            ],
            |_| None,
        )
        .unwrap();

        assert_eq!(config.planner_provider, Some(Provider::Gemini));
        assert_eq!(config.planner_model.as_deref(), Some("gemini-3.5-flash"));
        assert_eq!(config.resume.as_deref(), Some("session-1"));
        assert!(config.offline);
    }

    #[test]
    fn sidecar_option_is_not_a_config_key() {
        assert_eq!(cli_key("--sidecar-model"), None);
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "commandagent-config-test-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join(".commandagent")).unwrap();
        path
    }

    fn write_config(cwd: &Path, contents: &str) {
        let path = cwd.join(".commandagent/config");
        let mut file = fs::File::create(path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    }
}
