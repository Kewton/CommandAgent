use std::ffi::OsStr;
use std::path::Path;

use serde_json::json;

use super::{
    COMPATIBLE_REASONING_EFFORT_ENV, COMPATIBLE_REASONING_EFFORT_VALUES, CompatibleEffortSetting,
    declaration, invalid_message, role_applies,
};
use crate::config::{Config, ProviderRole};
use crate::providers::{openai_chat_completions, openai_responses};
use crate::state::ConversationMessage;
use crate::tools::registry::ToolRegistry;

fn parse(raw: &str) -> CompatibleEffortSetting {
    CompatibleEffortSetting::parse(Some(OsStr::new(raw)))
}

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

#[test]
fn unset_empty_and_whitespace_omit_the_field() {
    assert_eq!(
        CompatibleEffortSetting::parse(None),
        CompatibleEffortSetting::Omitted
    );
    for raw in ["", " ", "\t\n  "] {
        let setting = parse(raw);
        assert_eq!(setting, CompatibleEffortSetting::Omitted, "{raw:?}");
        assert_eq!(setting.for_client().unwrap(), None, "{raw:?}");
        assert_eq!(setting.as_str(), "omitted");
    }
}

#[test]
fn every_accepted_word_is_kept_verbatim_after_trimming() {
    assert_eq!(
        COMPATIBLE_REASONING_EFFORT_VALUES,
        ["none", "minimal", "low", "medium", "high", "xhigh", "max"]
    );
    for value in COMPATIBLE_REASONING_EFFORT_VALUES {
        assert_eq!(parse(value).for_client().unwrap(), Some(value));
        assert_eq!(
            parse(&format!("  {value}\t")).for_client().unwrap(),
            Some(value)
        );
    }
    assert_eq!(parse(" low ").as_str(), "declared");
    assert_eq!(parse(" low ").requested_value(), Some("low"));
}

#[test]
fn unknown_case_variant_boolean_and_alias_values_are_invalid() {
    for raw in [
        "LOW",
        "Low",
        "true",
        "false",
        "1",
        "off",
        "extra-high",
        "x-high",
        "default",
        "lowest",
        "low high",
        "\"low\"",
    ] {
        let setting = parse(raw);
        assert_eq!(setting, CompatibleEffortSetting::Invalid, "{raw:?}");
        assert_eq!(setting.as_str(), "invalid");
        assert_eq!(setting.requested_value(), None);
        let error = setting.for_client().unwrap_err().to_string();
        assert!(error.contains(COMPATIBLE_REASONING_EFFORT_ENV), "{error}");
        assert!(error.contains("xhigh"), "{error}");
        assert!(!format!("{setting:?}").contains(raw), "{raw:?}");
    }
}

#[cfg(unix)]
#[test]
fn non_unicode_value_is_invalid_and_never_rendered() {
    use std::os::unix::ffi::OsStrExt;

    let raw = OsStr::from_bytes(b"lo\xffw");
    let setting = CompatibleEffortSetting::parse(Some(raw));
    assert_eq!(setting, CompatibleEffortSetting::Invalid);
    assert!(setting.for_client().is_err());
}

#[test]
fn invalid_message_names_the_variable_and_vocabulary_only() {
    let message = invalid_message();
    assert!(message.contains(COMPATIBLE_REASONING_EFFORT_ENV));
    for value in COMPATIBLE_REASONING_EFFORT_VALUES {
        assert!(message.contains(value), "{message}");
    }
    assert!(message.contains("not shown"), "{message}");
}

#[test]
fn role_applies_to_generic_override_and_lm_studio_only() {
    let dir = tempfile::tempdir().unwrap();
    let executor_only = config(
        dir.path(),
        &[
            "--provider",
            "openai-compatible",
            "--base-url",
            "http://127.0.0.1:9/v1",
            "--model",
            "served",
            "--planner-provider",
            "ollama",
            "--planner-model",
            "planner",
        ],
    );
    assert!(role_applies(&executor_only, ProviderRole::Executor));
    assert!(!role_applies(&executor_only, ProviderRole::Planner));

    let planner_only = config(
        dir.path(),
        &[
            "--provider",
            "ollama",
            "--model",
            "executor",
            "--planner-provider",
            "lm-studio",
            "--planner-model",
            "planner",
        ],
    );
    assert!(!role_applies(&planner_only, ProviderRole::Executor));
    assert!(role_applies(&planner_only, ProviderRole::Planner));

    for provider in ["ollama", "openai", "gemini"] {
        let native = config(dir.path(), &["--provider", provider, "--model", "m"]);
        assert!(
            !role_applies(&native, ProviderRole::Executor),
            "{provider} must not receive the compatible effort"
        );
        assert!(!role_applies(&native, ProviderRole::Planner), "{provider}");
    }
}

#[test]
fn declaration_separates_declared_omitted_invalid_and_not_applicable_per_role() {
    let dir = tempfile::tempdir().unwrap();
    let both = config(
        dir.path(),
        &[
            "--provider",
            "openai-compatible",
            "--planner-provider",
            "openai-compatible",
            "--base-url",
            "http://127.0.0.1:9/v1",
            "--model",
            "served",
        ],
    );
    let declared = declaration(&both, parse("max"));
    assert_eq!(declared["status"], "declared");
    assert_eq!(declared["declared"], true);
    assert_eq!(declared["requested_value"], "max");
    assert_eq!(declared["source"], "env");
    assert_eq!(declared["server_support_verified"], false);
    assert_eq!(
        declared["applicable_roles"],
        json!(["executor", "planner", "classifier"])
    );
    for role in declared["roles"].as_array().unwrap() {
        assert_eq!(role["status"], "declared", "{role}");
        assert_eq!(role["requested_value"], "max", "{role}");
        assert_eq!(role["provider"], "openai-compatible", "{role}");
    }

    let omitted = declaration(&both, CompatibleEffortSetting::Omitted);
    assert_eq!(omitted["status"], "omitted");
    assert_eq!(omitted["declared"], false);
    assert!(omitted["requested_value"].is_null());
    assert!(omitted["source"].is_null());

    let invalid = declaration(&both, CompatibleEffortSetting::Invalid);
    assert_eq!(invalid["status"], "invalid");
    assert!(invalid["requested_value"].is_null());
    assert_eq!(invalid["roles"][0]["status"], "invalid");

    let native = config(dir.path(), &["--provider", "ollama", "--model", "m"]);
    let not_applicable = declaration(&native, parse("low"));
    assert_eq!(not_applicable["status"], "not_applicable");
    assert_eq!(not_applicable["declared"], false);
    assert_eq!(not_applicable["setting"], "declared");
    assert!(not_applicable["requested_value"].is_null());
    assert_eq!(not_applicable["roles"][0]["status"], "not_applicable");
}

#[test]
fn chat_builder_places_top_level_effort_once_and_otherwise_matches_the_legacy_request() {
    let messages = [ConversationMessage::user("hello")];
    let registry = ToolRegistry::default();
    let legacy = openai_chat_completions::build_lm_studio_request(
        "served",
        &messages,
        registry.specs(),
        true,
        128,
    );
    let declared = openai_chat_completions::build_lm_studio_request_with_effort(
        "served",
        &messages,
        registry.specs(),
        true,
        128,
        "low",
    );
    assert!(legacy.get("reasoning_effort").is_none());
    assert!(legacy.get("reasoning").is_none());
    assert_eq!(declared["reasoning_effort"], "low");
    assert!(declared.get("reasoning").is_none());
    assert!(declared.get("think").is_none());
    assert!(declared.get("max_completion_tokens").is_none());
    assert_eq!(declared["max_tokens"], 128);
    let mut without = declared.clone();
    without.as_object_mut().unwrap().remove("reasoning_effort");
    assert_eq!(without, legacy);
}

#[test]
fn responses_builder_places_nested_effort_once_and_otherwise_matches_the_legacy_request() {
    let messages = [ConversationMessage::user("hello")];
    let state = openai_responses::ConversationState::default();
    let legacy =
        openai_responses::build_lm_studio_request("served", &messages, &[], false, 128, &state);
    let declared = openai_responses::build_lm_studio_request_with_effort(
        "served",
        &messages,
        &[],
        false,
        128,
        "high",
        &state,
    );
    assert!(legacy.get("reasoning").is_none());
    assert_eq!(declared["reasoning"], json!({"effort": "high"}));
    assert!(declared.get("reasoning_effort").is_none());
    assert!(declared.get("think").is_none());
    assert!(declared.get("store").is_none());
    assert!(declared.get("include").is_none());
    assert_eq!(declared["max_output_tokens"], 128);
    let mut without = declared.clone();
    without.as_object_mut().unwrap().remove("reasoning");
    assert_eq!(without, legacy);
}
