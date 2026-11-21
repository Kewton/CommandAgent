//! `config.compatible_reasoning_effort`: the declared OpenAI-compatible
//! reasoning effort per real provider role (Issue #931). The check sends no
//! provider request; it reports what CommandAgent would send, never that the
//! server supports or applied it, and it never shows a rejected value.

use serde_json::Value;

use crate::config::Config;
use crate::providers::compatible_effort::{
    COMPATIBLE_REASONING_EFFORT_ENV, COMPATIBLE_REASONING_EFFORT_VALUES, CompatibleEffortSetting,
    declaration, invalid_message,
};

use super::{CheckStatus, DoctorCheck};

pub(super) fn compatible_reasoning_effort_check(config: &Config) -> DoctorCheck {
    check_with(config, CompatibleEffortSetting::from_process_env())
}

fn check_with(config: &Config, setting: CompatibleEffortSetting) -> DoctorCheck {
    let details = declaration(config, setting);
    let roles = string_list(&details["applicable_roles"]);
    let (status, message, remediation) = match details["status"].as_str() {
        Some("declared") => (
            CheckStatus::Pass,
            format!(
                "{} (declared via {COMPATIBLE_REASONING_EFFORT_ENV}; request field sent for {roles}; server support not verified)",
                setting.requested_value().unwrap_or_default()
            ),
            None,
        ),
        Some("invalid") => (
            CheckStatus::Fail,
            invalid_message(),
            Some(format!(
                "set {COMPATIBLE_REASONING_EFFORT_ENV} to one of {} or unset it, then rerun --doctor",
                COMPATIBLE_REASONING_EFFORT_VALUES.join(", ")
            )),
        ),
        Some("omitted") => (
            CheckStatus::Pass,
            format!("omitted (request field absent for {roles})"),
            None,
        ),
        _ => (
            CheckStatus::Pass,
            "not applicable (no OpenAI-compatible or LM Studio role)".to_string(),
            None,
        ),
    };
    DoctorCheck::new(
        "config.compatible_reasoning_effort",
        "configuration",
        "Compatible reasoning effort",
        status,
        message,
        remediation,
        details,
    )
}

fn string_list(value: &Value) -> String {
    value
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::Path;

    use super::{CheckStatus, CompatibleEffortSetting, check_with};
    use crate::config::Config;

    fn config(root: &Path, extra: &[&str]) -> Config {
        let root_arg = root.to_string_lossy().into_owned();
        let state_arg = root.join("state").to_string_lossy().into_owned();
        let mut arguments = vec![
            "commandagent",
            "--cwd",
            root_arg.as_str(),
            "--state-dir",
            state_arg.as_str(),
        ];
        arguments.extend_from_slice(extra);
        let parsed = crate::provider_cli::parse_from(arguments).expect("arguments");
        Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options).expect("config")
    }

    fn setting(raw: &str) -> CompatibleEffortSetting {
        CompatibleEffortSetting::parse(Some(OsStr::new(raw)))
    }

    const MIXED: &[&str] = &[
        "--provider",
        "openai-compatible",
        "--base-url",
        "http://127.0.0.1:9/v1",
        "--model",
        "served-model",
        "--planner-provider",
        "ollama",
        "--planner-model",
        "planner-model",
    ];

    #[test]
    fn declared_value_names_only_the_compatible_roles_and_stays_unverified() {
        let dir = tempfile::tempdir().unwrap();
        let check = check_with(&config(dir.path(), MIXED), setting(" low "));

        assert_eq!(check.id, "config.compatible_reasoning_effort");
        assert_eq!(check.status, CheckStatus::Pass);
        assert_eq!(check.details["status"], "declared");
        assert_eq!(check.details["declared"], true);
        assert_eq!(check.details["requested_value"], "low");
        assert_eq!(check.details["source"], "env");
        assert_eq!(check.details["server_support_verified"], false);
        assert_eq!(check.details["roles"][0]["role"], "executor");
        assert_eq!(check.details["roles"][0]["provider"], "openai-compatible");
        assert_eq!(check.details["roles"][0]["status"], "declared");
        assert_eq!(check.details["roles"][1]["role"], "planner");
        assert_eq!(check.details["roles"][1]["provider"], "ollama");
        assert_eq!(check.details["roles"][1]["status"], "not_applicable");
        assert!(check.details["roles"][1]["requested_value"].is_null());
        assert!(
            check.message.starts_with("low (declared"),
            "{}",
            check.message
        );
        assert!(check.message.contains("server support not verified"));
        assert!(!check.message.contains("planner"), "{}", check.message);
    }

    #[test]
    fn omitted_and_not_applicable_are_distinct_passes() {
        let dir = tempfile::tempdir().unwrap();
        let omitted = check_with(&config(dir.path(), MIXED), CompatibleEffortSetting::Omitted);
        assert_eq!(omitted.status, CheckStatus::Pass);
        assert_eq!(omitted.details["status"], "omitted");
        assert_eq!(omitted.details["declared"], false);
        assert!(omitted.details["requested_value"].is_null());
        assert!(
            omitted.message.starts_with("omitted"),
            "{}",
            omitted.message
        );

        let native = check_with(
            &config(dir.path(), &["--provider", "ollama", "--model", "m"]),
            setting("high"),
        );
        assert_eq!(native.status, CheckStatus::Pass);
        assert_eq!(native.details["status"], "not_applicable");
        assert_eq!(native.details["setting"], "declared");
        assert_eq!(native.details["declared"], false);
        assert!(native.details["requested_value"].is_null());
        assert_eq!(native.details["applicable_roles"], serde_json::json!([]));
    }

    #[test]
    fn invalid_value_fails_only_where_it_applies_and_is_never_shown() {
        let dir = tempfile::tempdir().unwrap();
        let rejected = "Turbo-931-raw";
        let invalid = check_with(&config(dir.path(), MIXED), setting(rejected));
        assert_eq!(invalid.status, CheckStatus::Fail);
        assert_eq!(invalid.details["status"], "invalid");
        assert!(invalid.details["requested_value"].is_null());
        assert!(
            invalid
                .message
                .contains("COMMANDAGENT_COMPATIBLE_REASONING_EFFORT")
        );
        assert!(invalid.remediation.as_deref().unwrap().contains("xhigh"));
        let rendered = serde_json::to_string(&invalid).unwrap();
        assert!(!rendered.contains(rejected), "{rendered}");

        let native = check_with(
            &config(dir.path(), &["--provider", "ollama", "--model", "m"]),
            setting(rejected),
        );
        assert_eq!(native.status, CheckStatus::Pass);
        assert_eq!(native.details["status"], "not_applicable");
        assert_eq!(native.details["setting"], "invalid");
        assert!(!serde_json::to_string(&native).unwrap().contains(rejected));
    }
}
