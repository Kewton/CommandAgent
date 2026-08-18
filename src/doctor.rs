use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::Cli;
use crate::config::{Config, OllamaThink, OpenAiApi, Provider};
use crate::minimal_loop::interaction_probe::{
    INTERACTION_PROBE_SETUP_REMEDIATION, ProbeAvailability,
};

const SCHEMA_VERSION: &str = "1";
const OLLAMA_TIMEOUT: Duration = Duration::from_secs(2);
const OPENAI_ENDPOINT: &str = "https://api.openai.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

impl CheckStatus {
    fn symbol(self) -> &'static str {
        match self {
            Self::Pass => "✓",
            Self::Warn => "!",
            Self::Fail => "✗",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DoctorCheck {
    pub id: String,
    pub category: String,
    pub label: String,
    pub status: CheckStatus,
    pub message: String,
    pub remediation: Option<String>,
    pub details: Value,
}

impl DoctorCheck {
    fn new(
        id: impl Into<String>,
        category: impl Into<String>,
        label: impl Into<String>,
        status: CheckStatus,
        message: impl Into<String>,
        remediation: Option<String>,
        details: Value,
    ) -> Self {
        Self {
            id: id.into(),
            category: category.into(),
            label: label.into(),
            status,
            message: single_line(message.into()),
            remediation: remediation.map(single_line),
            details,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DoctorReport {
    pub schema_version: &'static str,
    pub status: CheckStatus,
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    fn from_checks(checks: Vec<DoctorCheck>) -> Self {
        let status = if checks.iter().any(|check| check.status == CheckStatus::Fail) {
            CheckStatus::Fail
        } else if checks.iter().any(|check| check.status == CheckStatus::Warn) {
            CheckStatus::Warn
        } else {
            CheckStatus::Pass
        };
        Self {
            schema_version: SCHEMA_VERSION,
            status,
            checks,
        }
    }

    pub fn failed(&self) -> bool {
        self.status == CheckStatus::Fail
    }

    pub fn render_human(&self) -> String {
        let width = self
            .checks
            .iter()
            .map(|check| check.label.chars().count())
            .max()
            .unwrap_or(0);
        let mut lines = vec![format!(
            "CommandAgent doctor: {}",
            status_label(self.status)
        )];
        for check in &self.checks {
            lines.push(format!(
                "{} {:width$}  {}",
                check.status.symbol(),
                check.label,
                check.message,
                width = width
            ));
            if check.status != CheckStatus::Pass
                && let Some(remediation) = &check.remediation
            {
                lines.push(format!("  Remediation: {remediation}"));
            }
        }
        lines.join("\n")
    }

    pub fn render_json(&self) -> anyhow::Result<String> {
        serde_json::to_string_pretty(self).context("failed to serialize doctor report")
    }
}

pub fn run_cli(cli: Cli) -> anyhow::Result<()> {
    let render_json = cli.json;
    let report = diagnose_cli(&cli)?;
    if render_json {
        println!("{}", report.render_json()?);
    } else {
        println!("{}", report.render_human());
    }
    if report.failed() {
        anyhow::bail!("doctor found failed checks");
    }
    Ok(())
}

pub fn diagnose_cli(cli: &Cli) -> anyhow::Result<DoctorReport> {
    let requested_root = cli
        .cwd
        .clone()
        .unwrap_or(std::env::current_dir().context("failed to read current directory")?);
    let root = requested_root
        .canonicalize()
        .unwrap_or_else(|_| requested_root.clone());
    let resolved = Config::from_cli(cli.clone());
    let fallback_state_dir = cli
        .state_dir
        .clone()
        .unwrap_or_else(crate::config::default_state_dir);
    let resolution_error = resolved.as_ref().err().map(|error| format!("{error:#}"));
    Ok(collect_report(
        &root,
        resolved.as_ref().ok(),
        resolution_error.as_deref(),
        cli.preset.as_deref(),
        &fallback_state_dir,
    ))
}

pub fn diagnose(config: &Config) -> DoctorReport {
    let preset = selected_preset(config);
    collect_report(
        &config.workspace_root,
        Some(config),
        None,
        preset.as_deref(),
        &config.state_dir,
    )
}

fn collect_report(
    root: &Path,
    resolved: Option<&Config>,
    resolution_error: Option<&str>,
    preset_name: Option<&str>,
    fallback_state_dir: &Path,
) -> DoctorReport {
    let mut checks = Vec::new();
    match resolved {
        Some(config) => add_resolved_configuration_checks(&mut checks, config),
        None => checks.push(DoctorCheck::new(
            "config.resolution",
            "configuration",
            "Configuration",
            CheckStatus::Fail,
            resolution_error.unwrap_or("configuration could not be resolved"),
            Some("fix the reported config or preset problem, then rerun --doctor".to_string()),
            json!({ "error": resolution_error.unwrap_or("unknown") }),
        )),
    }
    add_config_file_checks(&mut checks, root, preset_name, resolution_error.is_some());
    if let Some(config) = resolved {
        add_provider_checks(&mut checks, config);
    }
    add_interaction_probe_check(&mut checks, root);
    let state_dir = resolved
        .map(|config| config.state_dir.as_path())
        .unwrap_or(fallback_state_dir);
    checks.push(writable_directory_check(
        "state.directory_writable",
        "state",
        "State directory",
        state_dir,
        "create the state directory and grant the current user write access",
    ));
    add_terminal_checks(&mut checks, resolved);
    checks.push(writable_directory_check(
        "workspace.root_writable",
        "workspace",
        "Workspace",
        root,
        "grant the current user write access or select a writable --cwd",
    ));
    checks.push(dotenv_check(root));
    DoctorReport::from_checks(checks)
}

fn add_resolved_configuration_checks(checks: &mut Vec<DoctorCheck>, config: &Config) {
    add_setting_check(
        checks,
        "config.model",
        "Model",
        &config.model,
        &config.field_sources.model,
    );
    add_setting_check(
        checks,
        "config.provider",
        "Provider",
        provider_label(config.provider),
        &config.field_sources.provider,
    );
    add_setting_check(
        checks,
        "config.planner_model",
        "Planner model",
        &config.planner_model,
        &config.field_sources.planner_model,
    );
    add_setting_check(
        checks,
        "config.planner_provider",
        "Planner provider",
        provider_label(config.planner_provider),
        &config.field_sources.planner_provider,
    );
    add_setting_check(
        checks,
        "config.profile",
        "Profile",
        &config.profile,
        &config.field_sources.profile,
    );
    checks.push(ollama_think_check(
        config.provider,
        config.planner_provider,
        config.ollama_think,
    ));
}

fn ollama_think_check(
    provider: Provider,
    planner_provider: Provider,
    configured: Option<OllamaThink>,
) -> DoctorCheck {
    let think = configured.map(OllamaThink::as_str);
    let roles = [
        (provider == Provider::Ollama).then_some("executor"),
        (planner_provider == Provider::Ollama).then_some("planner"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    DoctorCheck::new(
        "config.ollama_think",
        "configuration",
        "Ollama think",
        CheckStatus::Pass,
        match think {
            Some(value) => format!(
                "{value} (explicit; request field present for {})",
                roles.join(", ")
            ),
            None => "omitted (request field absent)".to_string(),
        },
        None,
        json!({
            "declared": think.is_some(),
            "effective_value": think,
            "request_field_present": think.is_some(),
            "ollama_roles": roles,
        }),
    )
}

fn add_setting_check(
    checks: &mut Vec<DoctorCheck>,
    id: &str,
    label: &str,
    value: &str,
    source_detail: &str,
) {
    let source = source_class(source_detail);
    checks.push(DoctorCheck::new(
        id,
        "configuration",
        label,
        CheckStatus::Pass,
        format!("{value} (source={source}; detail={source_detail})"),
        None,
        json!({
            "value": value,
            "source": source.to_ascii_lowercase(),
            "source_detail": source_detail,
        }),
    ));
}

fn add_config_file_checks(
    checks: &mut Vec<DoctorCheck>,
    root: &Path,
    preset_name: Option<&str>,
    resolution_failed: bool,
) {
    let inspection = crate::config::inspect_config_files(root, preset_name);
    const IDS: [&str; 4] = [
        "config.file.workspace_commandagent",
        "config.file.workspace_anvil",
        "config.file.home_commandagent",
        "config.file.home_anvil",
    ];
    for (index, file) in inspection.paths.into_iter().enumerate() {
        let id = IDS.get(index).copied().unwrap_or("config.file.additional");
        let path = file.path.display().to_string();
        if let Some(error) = file.parse_error {
            checks.push(DoctorCheck::new(
                id,
                "config_file",
                "Config file",
                CheckStatus::Fail,
                format!("{path}: invalid ({error})"),
                Some(format!(
                    "fix the syntax or unsupported key in {path}, then rerun --doctor"
                )),
                json!({ "path": path, "exists": true, "parseable": false, "error": error }),
            ));
        } else {
            checks.push(DoctorCheck::new(
                id,
                "config_file",
                "Config file",
                CheckStatus::Pass,
                if file.exists {
                    format!("{path}: present and parseable")
                } else {
                    format!("{path}: not found (optional)")
                },
                None,
                json!({ "path": path, "exists": file.exists, "parseable": file.exists }),
            ));
        }
    }
    if let Some(preset) = inspection.preset {
        let missing_keys = preset.missing_keys.join(", ");
        let (status, message, remediation) = if !preset.found {
            (
                CheckStatus::Fail,
                format!("preset '{}' was not found", preset.name),
                Some(format!(
                    "define [preset.{}] in a searched config file or select an existing preset",
                    preset.name
                )),
            )
        } else if !preset.complete && resolution_failed {
            (
                CheckStatus::Fail,
                format!(
                    "preset '{}' is incomplete and resolution failed; missing keys: {missing_keys}",
                    preset.name
                ),
                Some(format!(
                    "define the missing preset keys for '{}' or supply equivalent CLI values",
                    preset.name
                )),
            )
        } else if !preset.complete {
            (
                CheckStatus::Pass,
                format!(
                    "preset '{}' resolved with fallbacks; keys not defined by the preset: {missing_keys}",
                    preset.name
                ),
                None,
            )
        } else {
            (
                CheckStatus::Pass,
                format!("preset '{}' is complete", preset.name),
                None,
            )
        };
        checks.push(DoctorCheck::new(
            "config.preset",
            "configuration",
            "Preset",
            status,
            message,
            remediation,
            json!({
                "name": preset.name,
                "found": preset.found,
                "complete": preset.complete,
                "missing_keys": preset.missing_keys,
            }),
        ));
    }
}

fn add_provider_checks(checks: &mut Vec<DoctorCheck>, config: &Config) {
    let ollama_roles = [
        ("executor", config.provider, config.model.as_str()),
        (
            "planner",
            config.planner_provider,
            config.planner_model.as_str(),
        ),
    ]
    .into_iter()
    .filter_map(|(role, provider, model)| (provider == Provider::Ollama).then_some((role, model)))
    .collect::<Vec<_>>();
    if !ollama_roles.is_empty() {
        checks.extend(ollama_checks(&config.ollama_host, &ollama_roles));
    }

    let lm_studio_roles = [
        ("executor", config.provider, config.model.as_str()),
        (
            "planner",
            config.planner_provider,
            config.planner_model.as_str(),
        ),
    ]
    .into_iter()
    .filter_map(|(role, provider, model)| (provider == Provider::LmStudio).then_some((role, model)))
    .collect::<Vec<_>>();
    if !lm_studio_roles.is_empty() {
        let api_token =
            crate::env_compat::var(crate::providers::lm_studio::LM_STUDIO_API_TOKEN_ENV)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        checks.push(DoctorCheck::new(
            "provider.lm_studio.api_token",
            "provider",
            "LM Studio token",
            CheckStatus::Pass,
            if api_token.is_some() {
                "configured in the process environment"
            } else {
                "not configured (optional unless server authentication is enabled)"
            },
            None,
            json!({ "configured": api_token.is_some(), "source": "process_environment" }),
        ));
        checks.extend(lm_studio_checks(
            &config.lm_studio_host,
            api_token,
            &lm_studio_roles,
        ));
    }

    if config.provider == Provider::Openai || config.planner_provider == Provider::Openai {
        for (role, provider, model) in [
            ("executor", config.provider, config.model.as_str()),
            (
                "planner",
                config.planner_provider,
                config.planner_model.as_str(),
            ),
        ] {
            if provider == Provider::Openai {
                checks.push(openai_model_identity_check(role, model));
            }
        }
        let openai_key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty());
        checks.push(process_credential_check_with(
            "OPENAI_API_KEY",
            "provider.openai.api_key",
            "OpenAI key",
            |_| openai_key.clone(),
        ));
        checks.push(if config.openai_api == OpenAiApi::Responses {
            openai_responses_reachability_check(openai_key.as_deref())
        } else {
            openai_reachability_check(openai_key.as_deref())
        });
    }
    if config.provider == Provider::Gemini || config.planner_provider == Provider::Gemini {
        checks.push(credential_check_with(
            "GEMINI_API_KEY",
            "provider.gemini.api_key",
            "Gemini key",
            &crate::config::read_dotenv(&config.workspace_root),
            |name| std::env::var(name).ok(),
        ));
    }
}

fn openai_model_identity_check(role: &str, model: &str) -> DoctorCheck {
    let Some(identity) = crate::openai_model::identity(model) else {
        return DoctorCheck::new(
            format!("provider.openai.{role}_model_identity"),
            "provider",
            format!("OpenAI {role} model identity"),
            CheckStatus::Pass,
            format!("{model} is outside the registered GPT-5.6 family policy"),
            None,
            json!({
                "role": role,
                "requested_model": model,
                "registered_family": false,
                "snapshot_pinned": null,
            }),
        );
    };
    DoctorCheck::new(
        format!("provider.openai.{role}_model_identity"),
        "provider",
        format!("OpenAI {role} model identity"),
        CheckStatus::Pass,
        if identity.snapshot_pinned {
            format!("{model} is a date-qualified {} snapshot", identity.family_id)
        } else {
            format!("{model} is an exact model ID; no snapshot pin is declared")
        },
        (!identity.snapshot_pinned).then(|| {
            format!(
                "prefer a provider-published {} date snapshot when one is available for repeatable measurement",
                identity.family_id
            )
        }),
        json!({
            "role": role,
            "requested_model": model,
            "registered_family": true,
            "family_id": identity.family_id,
            "strict_id": true,
            "snapshot_pinned": identity.snapshot_pinned,
        }),
    )
}

fn openai_reachability_check(api_key: Option<&str>) -> DoctorCheck {
    openai_reachability_check_with(api_key, |key| {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(OLLAMA_TIMEOUT)
            .timeout(OLLAMA_TIMEOUT)
            .build()
            .map_err(|error| error.to_string())?;
        client
            .get(format!("{OPENAI_ENDPOINT}/v1/models"))
            .bearer_auth(key)
            .send()
            .map(|response| response.status().as_u16())
            .map_err(|error| error.to_string())
    })
}

fn openai_responses_reachability_check(api_key: Option<&str>) -> DoctorCheck {
    openai_responses_reachability_check_with(api_key, |key| {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(OLLAMA_TIMEOUT)
            .timeout(OLLAMA_TIMEOUT)
            .build()
            .map_err(|error| error.to_string())?;
        client
            .post(format!("{OPENAI_ENDPOINT}/v1/responses"))
            .bearer_auth(key)
            // Deliberately incomplete and therefore non-billable: HTTP 400
            // proves the authenticated Responses route exists without a turn.
            .json(&json!({}))
            .send()
            .map(|response| response.status().as_u16())
            .map_err(|error| error.to_string())
    })
}

fn openai_responses_reachability_check_with(
    api_key: Option<&str>,
    probe: impl FnOnce(&str) -> Result<u16, String>,
) -> DoctorCheck {
    let endpoint = format!("{OPENAI_ENDPOINT}/v1/responses");
    let Some(api_key) = api_key else {
        return DoctorCheck::new(
            "provider.openai.responses_reachable",
            "provider",
            "OpenAI Responses",
            CheckStatus::Warn,
            "reachability was not attempted because OPENAI_API_KEY is missing",
            Some("set OPENAI_API_KEY in the process environment and rerun --doctor".to_string()),
            json!({ "endpoint": endpoint, "reachable": null, "status": null }),
        );
    };
    match probe(api_key) {
        Ok(status) if (200..300).contains(&status) || status == 400 => DoctorCheck::new(
            "provider.openai.responses_reachable",
            "provider",
            "OpenAI Responses",
            CheckStatus::Pass,
            format!("{endpoint} reachable"),
            None,
            json!({ "endpoint": endpoint, "reachable": true, "status": status }),
        ),
        Ok(status) => DoctorCheck::new(
            "provider.openai.responses_reachable",
            "provider",
            "OpenAI Responses",
            CheckStatus::Warn,
            format!("{endpoint} reachable but returned HTTP {status}"),
            Some(
                "verify OpenAI Responses access, account permissions, and the process environment key"
                    .to_string(),
            ),
            json!({ "endpoint": endpoint, "reachable": true, "status": status }),
        ),
        Err(error) => {
            let error = single_line(error.replace(api_key, "<redacted>"));
            DoctorCheck::new(
                "provider.openai.responses_reachable",
                "provider",
                "OpenAI Responses",
                CheckStatus::Warn,
                format!("{endpoint} unreachable ({error})"),
                Some("verify network access to api.openai.com and rerun --doctor".to_string()),
                json!({
                    "endpoint": endpoint,
                    "reachable": false,
                    "status": null,
                    "error": error,
                }),
            )
        }
    }
}

fn openai_reachability_check_with(
    api_key: Option<&str>,
    probe: impl FnOnce(&str) -> Result<u16, String>,
) -> DoctorCheck {
    let Some(api_key) = api_key else {
        return DoctorCheck::new(
            "provider.openai.reachable",
            "provider",
            "OpenAI",
            CheckStatus::Warn,
            "reachability was not attempted because OPENAI_API_KEY is missing",
            Some("set OPENAI_API_KEY in the process environment and rerun --doctor".to_string()),
            json!({ "endpoint": OPENAI_ENDPOINT, "reachable": null, "status": null }),
        );
    };
    match probe(api_key) {
        Ok(status) if (200..300).contains(&status) => DoctorCheck::new(
            "provider.openai.reachable",
            "provider",
            "OpenAI",
            CheckStatus::Pass,
            format!("{OPENAI_ENDPOINT}/v1/models reachable"),
            None,
            json!({ "endpoint": OPENAI_ENDPOINT, "reachable": true, "status": status }),
        ),
        Ok(status) => DoctorCheck::new(
            "provider.openai.reachable",
            "provider",
            "OpenAI",
            CheckStatus::Warn,
            format!("{OPENAI_ENDPOINT}/v1/models reachable but returned HTTP {status}"),
            Some(
                "verify OpenAI access, account permissions, and the process environment key"
                    .to_string(),
            ),
            json!({ "endpoint": OPENAI_ENDPOINT, "reachable": true, "status": status }),
        ),
        Err(error) => {
            let error = single_line(error.replace(api_key, "<redacted>"));
            DoctorCheck::new(
                "provider.openai.reachable",
                "provider",
                "OpenAI",
                CheckStatus::Warn,
                format!("{OPENAI_ENDPOINT}/v1/models unreachable ({error})"),
                Some("verify network access to api.openai.com and rerun --doctor".to_string()),
                json!({
                    "endpoint": OPENAI_ENDPOINT,
                    "reachable": false,
                    "status": null,
                    "error": error,
                }),
            )
        }
    }
}

fn ollama_checks(host: &str, roles: &[(&str, &str)]) -> Vec<DoctorCheck> {
    let client = crate::providers::ollama::OllamaClient::new(
        host.to_string(),
        OLLAMA_TIMEOUT.as_secs(),
        1,
        0,
    );
    let models = client.and_then(|client| client.list_models());
    match models {
        Ok(models) => {
            let mut checks = vec![DoctorCheck::new(
                "provider.ollama.reachable",
                "provider",
                "Ollama",
                CheckStatus::Pass,
                format!("{host}/api/tags reachable; {} model tag(s)", models.len()),
                None,
                json!({ "host": host, "reachable": true, "tag_count": models.len() }),
            )];
            for (role, model) in roles {
                let present = models.iter().any(|candidate| candidate == model);
                checks.push(DoctorCheck::new(
                    format!("provider.ollama.{role}_model"),
                    "provider",
                    format!("Ollama {role} model"),
                    if present {
                        CheckStatus::Pass
                    } else {
                        CheckStatus::Fail
                    },
                    if present {
                        format!("{model} is present in /api/tags")
                    } else {
                        format!("{model} is absent from /api/tags")
                    },
                    (!present).then(|| {
                        format!(
                            "pull '{model}' on the configured Ollama host or choose an installed model"
                        )
                    }),
                    json!({ "role": role, "model": model, "present": present }),
                ));
            }
            checks
        }
        Err(error) => vec![DoctorCheck::new(
            "provider.ollama.reachable",
            "provider",
            "Ollama",
            CheckStatus::Fail,
            format!("{host}/api/tags unreachable ({error:#})"),
            Some(
                "start Ollama and verify --ollama-host, networking, and firewall settings"
                    .to_string(),
            ),
            json!({ "host": host, "reachable": false, "error": format!("{error:#}") }),
        )],
    }
}

fn lm_studio_checks(
    host: &str,
    api_token: Option<String>,
    roles: &[(&str, &str)],
) -> Vec<DoctorCheck> {
    let client = crate::providers::lm_studio::LmStudioClient::new(
        host.to_string(),
        api_token,
        OLLAMA_TIMEOUT.as_secs(),
        1,
        0,
        OpenAiApi::ChatCompletions,
        None,
    );
    let models = client.and_then(|client| client.list_models());
    match models {
        Ok(models) => {
            let mut checks = vec![DoctorCheck::new(
                "provider.lm_studio.reachable",
                "provider",
                "LM Studio",
                CheckStatus::Pass,
                format!(
                    "{host}/v1/models reachable; {} visible model(s)",
                    models.len()
                ),
                None,
                json!({ "host": host, "reachable": true, "model_count": models.len() }),
            )];
            for (role, model) in roles {
                let present = models.iter().any(|candidate| candidate == model);
                checks.push(DoctorCheck::new(
                    format!("provider.lm_studio.{role}_model"),
                    "provider",
                    format!("LM Studio {role} model"),
                    if present {
                        CheckStatus::Pass
                    } else {
                        CheckStatus::Fail
                    },
                    if present {
                        format!("{model} is visible in /v1/models")
                    } else {
                        format!("{model} is absent from /v1/models")
                    },
                    (!present).then(|| {
                        format!(
                            "load '{model}' in LM Studio, enable Just-In-Time loading, or choose a visible model"
                        )
                    }),
                    json!({ "role": role, "model": model, "present": present }),
                ));
            }
            checks
        }
        Err(error) => {
            let message = single_line(format!("{error:#}"));
            let authentication_failed = message.contains("401") || message.contains("403");
            vec![DoctorCheck::new(
                "provider.lm_studio.reachable",
                "provider",
                "LM Studio",
                CheckStatus::Fail,
                format!("{host}/v1/models unreachable ({message})"),
                Some(if authentication_failed {
                    format!(
                        "set {} in the process environment to a valid LM Studio API token",
                        crate::providers::lm_studio::LM_STUDIO_API_TOKEN_ENV
                    )
                } else {
                    "start the LM Studio server and verify --lm-studio-host, networking, and firewall settings"
                        .to_string()
                }),
                json!({
                    "host": host,
                    "reachable": false,
                    "authentication_failed": authentication_failed,
                    "error": message,
                }),
            )]
        }
    }
}

fn credential_check_with(
    key: &str,
    id: &str,
    label: &str,
    dotenv: &HashMap<String, String>,
    get_env: impl Fn(&str) -> Option<String>,
) -> DoctorCheck {
    let credential = get_env(key)
        .filter(|value| !value.trim().is_empty())
        .map(|value| (value, "environment"))
        .or_else(|| {
            dotenv
                .get(key)
                .filter(|value| !value.trim().is_empty())
                .cloned()
                .map(|value| (value, ".env"))
        });
    match credential {
        Some((value, source)) => {
            let redacted = crate::config::redact(&value);
            DoctorCheck::new(
                id,
                "provider",
                label,
                CheckStatus::Pass,
                format!("{key} is set (source={source}; value={redacted})"),
                None,
                json!({ "key": key, "present": true, "source": source, "value": redacted }),
            )
        }
        None => DoctorCheck::new(
            id,
            "provider",
            label,
            CheckStatus::Fail,
            format!("{key} is not set in the environment or workspace .env"),
            Some(format!(
                "set {key} in the process environment or workspace .env without printing it"
            )),
            json!({ "key": key, "present": false, "source": null, "value": null }),
        ),
    }
}

fn process_credential_check_with(
    key: &str,
    id: &str,
    label: &str,
    get_env: impl Fn(&str) -> Option<String>,
) -> DoctorCheck {
    match get_env(key).filter(|value| !value.trim().is_empty()) {
        Some(value) => {
            let redacted = crate::config::redact(&value);
            DoctorCheck::new(
                id,
                "provider",
                label,
                CheckStatus::Pass,
                format!("{key} is set (source=environment; value={redacted})"),
                None,
                json!({ "key": key, "present": true, "source": "environment", "value": redacted }),
            )
        }
        None => DoctorCheck::new(
            id,
            "provider",
            label,
            CheckStatus::Fail,
            format!("{key} is not set in the process environment"),
            Some(format!(
                "set {key} in the process environment without printing it"
            )),
            json!({ "key": key, "present": false, "source": null, "value": null }),
        ),
    }
}

fn add_interaction_probe_check(checks: &mut Vec<DoctorCheck>, root: &Path) {
    match crate::minimal_loop::interaction_probe::playwright_availability(root) {
        ProbeAvailability::Available(resolution) => checks.push(DoctorCheck::new(
            "interaction.playwright",
            "interaction_probe",
            "Playwright probe",
            CheckStatus::Pass,
            format!(
                "playwright {} available ({})",
                resolution.version, resolution.location
            ),
            None,
            json!({
                "available": true,
                "version": resolution.version,
                "location": resolution.location,
                "module_path": resolution.module_path,
            }),
        )),
        ProbeAvailability::Unavailable(reason) => checks.push(DoctorCheck::new(
            "interaction.playwright",
            "interaction_probe",
            "Playwright probe",
            CheckStatus::Warn,
            format!("unavailable ({reason})"),
            Some(INTERACTION_PROBE_SETUP_REMEDIATION.to_string()),
            json!({ "available": false, "reason": reason }),
        )),
    }
}

fn writable_directory_check(
    id: &str,
    category: &str,
    label: &str,
    path: &Path,
    remediation: &str,
) -> DoctorCheck {
    match probe_directory_write(path) {
        Ok(()) => DoctorCheck::new(
            id,
            category,
            label,
            CheckStatus::Pass,
            format!(
                "{} is writable (temporary file created and removed)",
                path.display()
            ),
            None,
            json!({ "path": path, "writable": true, "probe_file_removed": true }),
        ),
        Err(error) => DoctorCheck::new(
            id,
            category,
            label,
            CheckStatus::Fail,
            format!("{} is not writable ({error:#})", path.display()),
            Some(remediation.to_string()),
            json!({ "path": path, "writable": false, "error": format!("{error:#}") }),
        ),
    }
}

fn probe_directory_write(path: &Path) -> anyhow::Result<()> {
    if !path.is_dir() {
        anyhow::bail!("directory does not exist");
    }
    let probe_path = path.join(format!(".commandagent-doctor-{}.tmp", uuid::Uuid::now_v7()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe_path)
        .with_context(|| format!("failed to create {}", probe_path.display()))?;
    if let Err(error) = file.write_all(b"commandagent doctor write probe\n") {
        drop(file);
        let _ = fs::remove_file(&probe_path);
        return Err(error).context("failed to write temporary probe file");
    }
    drop(file);
    fs::remove_file(&probe_path)
        .with_context(|| format!("failed to remove {}", probe_path.display()))?;
    Ok(())
}

fn add_terminal_checks(checks: &mut Vec<DoctorCheck>, config: Option<&Config>) {
    let stdin_tty = crate::tui::terminal::stdin_is_tty();
    let stdout_tty = crate::tui::terminal::stdout_is_tty();
    let stderr_tty = crate::tui::terminal::stderr_is_tty();
    let tty_ready = stdin_tty && stdout_tty;
    checks.push(DoctorCheck::new(
        "terminal.tty",
        "terminal",
        "TTY",
        if tty_ready {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        format!("stdin={stdin_tty}, stdout={stdout_tty}, stderr={stderr_tty}"),
        (!tty_ready)
            .then(|| "run from an interactive terminal when validating TUI behavior".to_string()),
        json!({ "stdin": stdin_tty, "stdout": stdout_tty, "stderr": stderr_tty }),
    ));

    let no_color = crate::tui::terminal::no_color();
    checks.push(DoctorCheck::new(
        "terminal.color",
        "terminal",
        "Color",
        CheckStatus::Pass,
        if no_color {
            "disabled because NO_COLOR is set"
        } else {
            "enabled (NO_COLOR is not set)"
        },
        None,
        json!({ "enabled": !no_color, "no_color": no_color }),
    ));

    let width = stdout_tty
        .then(|| crossterm::terminal::size().ok().map(|(width, _)| width))
        .flatten();
    checks.push(DoctorCheck::new(
        "terminal.width",
        "terminal",
        "Terminal width",
        if width.is_some() {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        width
            .map(|width| format!("{width} columns"))
            .unwrap_or_else(|| "unavailable".to_string()),
        width
            .is_none()
            .then(|| "run from a TTY whose terminal size can be queried".to_string()),
        json!({ "columns": width }),
    ));

    if let Some(config) = config {
        let footer = crate::tui::footer::FooterEnv::detect(config);
        let disabled_by_env = crate::tui::terminal::env_non_empty("COMMANDAGENT_NO_FOOTER");
        let reason = if footer.enabled {
            "enabled".to_string()
        } else if config.no_footer {
            "disabled by resolved footer configuration".to_string()
        } else if disabled_by_env {
            "disabled by COMMANDAGENT_NO_FOOTER (or legacy fallback)".to_string()
        } else if !stdout_tty {
            "disabled because stdout is not a TTY".to_string()
        } else {
            "disabled by terminal conditions".to_string()
        };
        checks.push(DoctorCheck::new(
            "terminal.footer",
            "terminal",
            "Footer",
            if footer.enabled {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            reason,
            (!footer.enabled).then(|| {
                "use a stdout TTY and remove footer-disable flags or environment settings if the fixed footer is desired"
                    .to_string()
            }),
            json!({ "enabled": footer.enabled, "color_enabled": footer.use_color }),
        ));
    } else {
        checks.push(DoctorCheck::new(
            "terminal.footer",
            "terminal",
            "Footer",
            CheckStatus::Warn,
            "cannot resolve footer readiness until configuration is valid",
            Some("fix configuration resolution, then rerun --doctor".to_string()),
            json!({ "enabled": null, "color_enabled": null }),
        ));
    }
}

fn dotenv_check(root: &Path) -> DoctorCheck {
    let path = root.join(".env");
    match fs::read_to_string(&path) {
        Ok(_) => {
            let mut keys = crate::config::read_dotenv(root)
                .into_keys()
                .filter(|key| !key.trim().is_empty())
                .collect::<Vec<_>>();
            keys.sort();
            DoctorCheck::new(
                "workspace.dotenv",
                "workspace",
                "Workspace .env",
                CheckStatus::Pass,
                if keys.is_empty() {
                    format!("{} is present; no keys are defined", path.display())
                } else {
                    format!("{} is present; keys: {}", path.display(), keys.join(", "))
                },
                None,
                json!({ "path": path, "exists": true, "keys": keys }),
            )
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => DoctorCheck::new(
            "workspace.dotenv",
            "workspace",
            "Workspace .env",
            CheckStatus::Pass,
            format!("{} is not present", path.display()),
            None,
            json!({ "path": path, "exists": false, "keys": [] }),
        ),
        Err(error) => DoctorCheck::new(
            "workspace.dotenv",
            "workspace",
            "Workspace .env",
            CheckStatus::Fail,
            format!("{} cannot be read ({error})", path.display()),
            Some(
                "grant the current user read access to .env or remove the unreadable file"
                    .to_string(),
            ),
            json!({ "path": path, "exists": true, "error": error.to_string() }),
        ),
    }
}

fn selected_preset(config: &Config) -> Option<String> {
    [
        &config.field_sources.model,
        &config.field_sources.provider,
        &config.field_sources.planner_model,
        &config.field_sources.planner_provider,
        &config.field_sources.profile,
    ]
    .into_iter()
    .find_map(|source| source.strip_prefix("preset:").map(ToString::to_string))
}

fn source_class(source: &str) -> &'static str {
    if source == "flag" {
        "CLI"
    } else if source.starts_with("preset:") {
        "preset"
    } else if source.starts_with("config:") {
        "config"
    } else {
        "default"
    }
}

fn provider_label(provider: Provider) -> &'static str {
    provider.as_str()
}

fn status_label(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Pass => "passed",
        CheckStatus::Warn => "warnings",
        CheckStatus::Fail => "failed",
    }
}

fn single_line(value: String) -> String {
    value.replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn aggregate_status_and_human_rendering_use_symbols_and_failure_remediation() {
        let report = DoctorReport::from_checks(vec![
            DoctorCheck::new(
                "ok",
                "test",
                "Short",
                CheckStatus::Pass,
                "ready",
                None,
                json!({}),
            ),
            DoctorCheck::new(
                "warn",
                "test",
                "Long label",
                CheckStatus::Warn,
                "degraded",
                Some("do the safe thing".to_string()),
                json!({}),
            ),
        ]);

        assert_eq!(report.status, CheckStatus::Warn);
        let text = report.render_human();
        assert!(text.contains("✓ Short"), "{text}");
        assert!(text.contains("! Long label"), "{text}");
        assert!(text.contains("Remediation: do the safe thing"), "{text}");
    }

    #[test]
    fn credential_prefers_environment_and_never_exposes_value() {
        let dotenv = HashMap::from([("OPENAI_API_KEY".to_string(), "dotenv-secret".to_string())]);
        let check = credential_check_with(
            "OPENAI_API_KEY",
            "provider.openai.api_key",
            "OpenAI key",
            &dotenv,
            |_| Some("environment-secret".to_string()),
        );
        let serialized = serde_json::to_string(&check).unwrap();

        assert_eq!(check.status, CheckStatus::Pass);
        assert!(check.message.contains("source=environment"));
        assert!(check.message.contains("<redacted>"));
        assert!(!serialized.contains("environment-secret"));
        assert!(!serialized.contains("dotenv-secret"));
    }

    #[test]
    fn credential_falls_back_to_dotenv_and_missing_is_failure() {
        let dotenv = HashMap::from([("GEMINI_API_KEY".to_string(), "dotenv-secret".to_string())]);
        let present = credential_check_with(
            "GEMINI_API_KEY",
            "provider.gemini.api_key",
            "Gemini key",
            &dotenv,
            |_| None,
        );
        let missing = credential_check_with(
            "OPENAI_API_KEY",
            "provider.openai.api_key",
            "OpenAI key",
            &dotenv,
            |_| None,
        );

        assert!(present.message.contains("source=.env"));
        assert_eq!(missing.status, CheckStatus::Fail);
    }

    #[test]
    fn openai_credential_is_environment_only() {
        let check = process_credential_check_with(
            "OPENAI_API_KEY",
            "provider.openai.api_key",
            "OpenAI key",
            |_| None,
        );

        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check.message.contains("process environment"));
        assert!(!check.message.contains(".env"));
    }

    #[test]
    fn openai_reachability_reports_status_without_exposing_key() {
        let secret = "sk-proj-doctor-secret-123456789";
        let reachable = openai_reachability_check_with(Some(secret), |_| Ok(200));
        let reflected = openai_reachability_check_with(Some(secret), |_| {
            Err(format!("upstream reflected {secret}"))
        });
        let serialized = serde_json::to_string(&(reachable.clone(), reflected.clone())).unwrap();

        assert_eq!(reachable.status, CheckStatus::Pass);
        assert_eq!(reflected.status, CheckStatus::Warn);
        assert!(serialized.contains("<redacted>"));
        assert!(!serialized.contains(secret));
    }

    #[test]
    fn responses_reachability_accepts_non_billable_probe_and_redacts_key() {
        let secret = "sk-proj-doctor-responses-secret-123456789";
        let reachable = openai_responses_reachability_check_with(Some(secret), |_| Ok(400));
        let reflected = openai_responses_reachability_check_with(Some(secret), |_| {
            Err(format!("upstream reflected {secret}"))
        });
        let serialized = serde_json::to_string(&(reachable.clone(), reflected.clone())).unwrap();

        assert_eq!(reachable.status, CheckStatus::Pass);
        assert_eq!(reflected.status, CheckStatus::Warn);
        assert!(serialized.contains("/v1/responses"));
        assert!(serialized.contains("<redacted>"));
        assert!(!serialized.contains(secret));
    }

    #[test]
    fn terra_doctor_identity_is_strict_and_recommends_snapshot_pin() {
        let exact = openai_model_identity_check("executor", "gpt-5.6-terra");
        let pinned = openai_model_identity_check("planner", "gpt-5.6-terra-2026-08-18");

        assert_eq!(exact.status, CheckStatus::Pass);
        assert_eq!(exact.details["strict_id"], true);
        assert_eq!(exact.details["snapshot_pinned"], false);
        assert!(
            exact
                .remediation
                .as_deref()
                .unwrap()
                .contains("provider-published")
        );
        assert_eq!(pinned.details["snapshot_pinned"], true);
        assert!(pinned.remediation.is_none());
    }

    #[test]
    fn ollama_think_metadata_distinguishes_omitted_and_explicit_values() {
        let omitted = ollama_think_check(Provider::Ollama, Provider::Openai, None);
        let explicit = ollama_think_check(
            Provider::Openai,
            Provider::Ollama,
            Some(OllamaThink::Medium),
        );

        assert_eq!(omitted.details["declared"], false);
        assert!(omitted.details["effective_value"].is_null());
        assert_eq!(omitted.details["request_field_present"], false);
        assert_eq!(omitted.details["ollama_roles"], json!(["executor"]));
        assert_eq!(explicit.details["effective_value"], "medium");
        assert_eq!(explicit.details["request_field_present"], true);
        assert_eq!(explicit.details["ollama_roles"], json!(["planner"]));
    }

    #[test]
    fn writable_probe_removes_its_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        probe_directory_write(dir.path()).unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn ollama_tags_check_covers_reachability_and_each_role_model() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let read = stream.read(&mut request).unwrap();
            assert!(String::from_utf8_lossy(&request[..read]).contains("GET /api/tags"));
            let body = r#"{"models":[{"name":"executor:latest"}]}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });

        let host = format!("http://{address}");
        let checks = ollama_checks(
            &host,
            &[
                ("executor", "executor:latest"),
                ("planner", "missing:latest"),
            ],
        );
        server.join().unwrap();

        assert_eq!(checks[0].status, CheckStatus::Pass);
        assert_eq!(checks[1].status, CheckStatus::Pass);
        assert_eq!(checks[2].status, CheckStatus::Fail);
    }

    #[test]
    fn lm_studio_models_check_covers_reachability_auth_and_role_models() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 2048];
            let read = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.contains("GET /v1/models"));
            assert!(request.contains("authorization: Bearer lm-doctor-token"));
            let body = r#"{"object":"list","data":[{"id":"executor/model"}]}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });

        let host = format!("http://{address}/v1");
        let checks = lm_studio_checks(
            &host,
            Some("lm-doctor-token".to_string()),
            &[("executor", "executor/model"), ("planner", "missing/model")],
        );
        server.join().unwrap();

        assert_eq!(checks[0].status, CheckStatus::Pass);
        assert_eq!(checks[1].status, CheckStatus::Pass);
        assert_eq!(checks[2].status, CheckStatus::Fail);
        assert!(checks[2].remediation.as_deref().unwrap().contains("load"));
    }

    #[test]
    fn lm_studio_auth_failure_has_token_remediation() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request).unwrap();
            write!(
                stream,
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
        });

        let checks = lm_studio_checks(
            &format!("http://{address}"),
            None,
            &[("executor", "executor/model")],
        );
        server.join().unwrap();

        assert_eq!(checks[0].status, CheckStatus::Fail);
        assert!(
            checks[0]
                .remediation
                .as_deref()
                .unwrap()
                .contains(crate::providers::lm_studio::LM_STUDIO_API_TOKEN_ENV)
        );
    }
}
