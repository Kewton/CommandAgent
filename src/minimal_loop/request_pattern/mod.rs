//! Request pattern routing for the direct `--prompt` entry (Issue #918).
//!
//! With `--request-pattern jev` the request text is classified once, before the
//! run, into ANSWER, CREATE, MODIFY, or INVESTIGATE, and an adopted pattern adds
//! a short "Request guidance" block to the system prompt. Every failure, an
//! unknown label, a missing key, `--offline`, and a confidence below the
//! threshold fall back to DEFAULT, which adds no guidance. The pattern only
//! selects guidance: tool permissions, required artifacts, completion,
//! contracts, assurance, and success evidence never read it. `off` (the
//! default), an explicit `--intent`, the plan entries, and the TUI never call
//! the classifier and leave the request byte-identical.

mod guidance;
mod jev;
#[cfg(test)]
mod tests;

use std::cell::Cell;
use std::marker::PhantomData;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::loop_run::{
    RunSessionOptions, RunSessionOutcome, run_session_with_outcome_with_options,
};
use super::prompt::PromptGuidance;
use crate::cli::RequestPatternArg;
use crate::config::{self, Action, Config};
use crate::providers::ChatClient;
use crate::state::SessionSnapshot;
use crate::tui::InteractionUi;

pub use guidance::GUIDANCE_VERSION;
pub use jev::{
    JEV_API_KEY_ENV, JEV_ENDPOINT, JEV_TIMEOUT, JevClassifier, parse_response, request_body,
};

pub const DEFAULT_THRESHOLD: f64 = 0.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestPatternMode {
    Off,
    Jev,
}

impl RequestPatternMode {
    pub fn from_config_value(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "jev" => Some(Self::Jev),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RequestPatternSettings {
    pub mode: RequestPatternMode,
    pub threshold: f64,
}

impl RequestPatternSettings {
    pub const OFF: Self = Self {
        mode: RequestPatternMode::Off,
        threshold: DEFAULT_THRESHOLD,
    };
}

impl Default for RequestPatternSettings {
    fn default() -> Self {
        Self::OFF
    }
}

/// A threshold is a finite number in `0..=1`.
pub fn parse_threshold(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|threshold| threshold.is_finite() && (0.0..=1.0).contains(threshold))
}

const MODE_KEY: &str = "request_pattern";
const THRESHOLD_KEY: &str = "request_pattern_threshold";

fn config_mode(path: &Path, line_no: usize, key: &str, value: &str) -> Result<RequestPatternMode> {
    let value = config::parse_string_value(path, line_no, key, value)?;
    RequestPatternMode::from_config_value(&value).ok_or_else(|| {
        anyhow::anyhow!(
            "{}:{} {key} expects request_pattern off|jev",
            path.display(),
            line_no
        )
    })
}

fn config_threshold(path: &Path, line_no: usize, key: &str, value: &str) -> Result<f64> {
    parse_threshold(value).ok_or_else(|| {
        anyhow::anyhow!(
            "{}:{} {key} expects a number from 0 to 1",
            path.display(),
            line_no
        )
    })
}

/// Checks one `request_pattern` or `request_pattern_threshold` value wherever
/// a config file is parsed. Diagnostics name the file, line and key only.
pub(crate) fn check_config_value(
    path: &Path,
    line_no: usize,
    key: &str,
    value: &str,
) -> Result<()> {
    if key == MODE_KEY {
        config_mode(path, line_no, key, value).map(drop)
    } else {
        config_threshold(path, line_no, key, value).map(drop)
    }
}

/// Resolves the settings: the `--request-pattern` flag, else the first config
/// layer that sets `request_pattern`, else `off`; the threshold comes from the
/// first layer that sets `request_pattern_threshold`, else 0.8. Only these two
/// top-level keys are read, and a bad value in either is a configuration error.
pub fn settings(flag: Option<RequestPatternArg>, root: &Path) -> Result<RequestPatternSettings> {
    let mut mode = None;
    let mut threshold = None;
    for path in config::config_paths(root) {
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(err).with_context(|| format!("failed to read {}", path.display()));
            }
        };
        for (index, raw_line) in text.lines().enumerate() {
            let line = config::strip_toml_comment(raw_line).trim();
            if line.starts_with('[') {
                break;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value, line_no) = (key.trim(), value.trim(), index + 1);
            if key == MODE_KEY && mode.is_none() {
                mode = Some(config_mode(&path, line_no, key, value)?);
            } else if key == THRESHOLD_KEY && threshold.is_none() {
                threshold = Some(config_threshold(&path, line_no, key, value)?);
            }
        }
    }
    let mode = match flag {
        Some(RequestPatternArg::Off) => RequestPatternMode::Off,
        Some(RequestPatternArg::Jev) => RequestPatternMode::Jev,
        None => mode.unwrap_or(RequestPatternMode::Off),
    };
    Ok(RequestPatternSettings {
        mode,
        threshold: threshold.unwrap_or(DEFAULT_THRESHOLD),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestPattern {
    Answer,
    Create,
    Modify,
    Investigate,
}

impl RequestPattern {
    pub const ALL: [Self; 4] = [Self::Answer, Self::Create, Self::Modify, Self::Investigate];

    pub fn label(self) -> &'static str {
        match self {
            Self::Answer => "ANSWER",
            Self::Create => "CREATE",
            Self::Modify => "MODIFY",
            Self::Investigate => "INVESTIGATE",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|pattern| pattern.label() == label)
    }

    pub fn guidance(self) -> &'static str {
        guidance::text(self)
    }
}

/// What a classifier answered. `pattern` is `None` for a label outside the four.
#[derive(Debug, Clone, PartialEq)]
pub struct Classification {
    pub pattern: Option<RequestPattern>,
    pub confidence: f64,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassifyFailure {
    Http,
    Timeout,
    Parse,
}

impl ClassifyFailure {
    fn reason(self) -> &'static str {
        match self {
            Self::Http => "http_error",
            Self::Timeout => "timeout",
            Self::Parse => "parse_error",
        }
    }
}

/// Classifies the request text. Implementations receive the request text only.
pub trait PatternClassifier {
    fn classify(&self, request: &str) -> Result<Classification, ClassifyFailure>;
}

/// The outcome of one routing decision; `pattern` is set only when adopted.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolution {
    pub pattern: Option<RequestPattern>,
    pub confidence: Option<f64>,
    pub reason: &'static str,
    pub model: Option<String>,
    pub latency_ms: u64,
}

impl Resolution {
    fn fallback(reason: &'static str) -> Self {
        Self {
            pattern: None,
            confidence: None,
            reason,
            model: None,
            latency_ms: 0,
        }
    }

    pub fn adopted(&self) -> bool {
        self.pattern.is_some()
    }

    pub fn guidance(&self) -> Option<&'static str> {
        self.pattern.map(RequestPattern::guidance)
    }

    /// The `request_pattern_resolved` event. It never carries the request text
    /// or the key.
    pub fn event(&self) -> Value {
        json!({
            "event": "request_pattern_resolved",
            "source": "jev",
            "pattern": self.pattern.map(RequestPattern::label).unwrap_or("default"),
            "confidence": self.confidence,
            "adopted": self.adopted(),
            "reason": self.reason,
            "model": self.model,
            "latency_ms": self.latency_ms,
            "guidance_version": self.adopted().then_some(GUIDANCE_VERSION),
        })
    }
}

/// Decides the pattern for `request`. A missing classifier means no key.
pub fn resolve(
    settings: RequestPatternSettings,
    request: &str,
    offline: bool,
    classifier: Option<&dyn PatternClassifier>,
) -> Resolution {
    if offline {
        return Resolution::fallback("offline");
    }
    let Some(classifier) = classifier else {
        return Resolution::fallback("missing_key");
    };
    let started = Instant::now();
    let answer = classifier.classify(request);
    let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let classification = match answer {
        Ok(classification) => classification,
        Err(failure) => {
            return Resolution {
                latency_ms,
                ..Resolution::fallback(failure.reason())
            };
        }
    };
    let (pattern, reason) = match classification.pattern {
        None => (None, "unknown_label"),
        Some(_) if classification.confidence < settings.threshold => (None, "below_threshold"),
        Some(pattern) => (Some(pattern), "adopted"),
    };
    Resolution {
        pattern,
        confidence: Some(classification.confidence),
        reason,
        model: classification.model,
        latency_ms,
    }
}

/// Whether a direct prompt run with `config` routes at all: only `jev` mode,
/// and never with an explicit `--intent`.
pub fn routes(settings: RequestPatternSettings, config: &Config) -> bool {
    settings.mode == RequestPatternMode::Jev && config.intent_override.is_none()
}

/// Runs the direct prompt with the installed settings and the env classifier.
pub(crate) fn run_direct_prompt(
    client: &mut dyn ChatClient,
    session: &mut SessionSnapshot,
    prompt: &str,
    config: &Config,
    ui: &dyn InteractionUi,
) -> anyhow::Result<RunSessionOutcome> {
    let settings = current();
    let classifier = routes(settings, config)
        .then(JevClassifier::from_env)
        .flatten();
    run_direct_prompt_with(
        client,
        session,
        prompt,
        config,
        ui,
        settings,
        classifier
            .as_ref()
            .map(|classifier| classifier as &dyn PatternClassifier),
    )
}

/// Runs the direct prompt. When routing applies, the decision is made once,
/// recorded as `request_pattern_resolved`, and its guidance (if adopted) goes
/// into the run options; otherwise the run is exactly the plain direct run.
pub fn run_direct_prompt_with(
    client: &mut dyn ChatClient,
    session: &mut SessionSnapshot,
    prompt: &str,
    config: &Config,
    ui: &dyn InteractionUi,
    settings: RequestPatternSettings,
    classifier: Option<&dyn PatternClassifier>,
) -> anyhow::Result<RunSessionOutcome> {
    let mut options = RunSessionOptions::default();
    if routes(settings, config) {
        let resolution = resolve(settings, prompt, config.offline, classifier);
        emit(config.eval_events_path.as_deref(), &resolution);
        options.request_guidance = resolution.guidance().map(str::to_string);
    }
    run_session_with_outcome_with_options(client, session, prompt, &[], config, ui, options)
}

fn emit(path: Option<&Path>, resolution: &Resolution) {
    crate::eval_events::emit(path, resolution.event());
}

impl RunSessionOptions {
    /// The system prompt guidance for this run: the profile block plus the
    /// request block a direct prompt run adopted.
    pub(crate) fn prompt_guidance<'a>(&'a self, profile: Option<&'a str>) -> PromptGuidance<'a> {
        PromptGuidance {
            profile,
            request: self.request_guidance.as_deref(),
        }
    }
}

thread_local! {
    static ACTIVE: Cell<RequestPatternSettings> = const { Cell::new(RequestPatternSettings::OFF) };
}

/// Restores the previous settings when dropped, on every exit path.
pub struct ScopedRequestPattern {
    previous: RequestPatternSettings,
    _not_send: PhantomData<Rc<()>>,
}

impl Drop for ScopedRequestPattern {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(self.previous));
    }
}

/// Installs `settings` for a top-level direct `--prompt` run. Every other
/// action (plan entries, the TUI, workflows) gets `off`.
pub fn install(action: &Action, settings: RequestPatternSettings) -> ScopedRequestPattern {
    let next = if matches!(action, Action::Prompt(_)) {
        settings
    } else {
        RequestPatternSettings::OFF
    };
    ScopedRequestPattern {
        previous: ACTIVE.with(|active| active.replace(next)),
        _not_send: PhantomData,
    }
}

/// The settings installed for the current run, `off` outside one.
pub fn current() -> RequestPatternSettings {
    ACTIVE.with(Cell::get)
}
