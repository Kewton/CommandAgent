use std::cell::Cell;
use std::path::Path;

use clap::Parser;

use super::{
    Classification, ClassifyFailure, GUIDANCE_VERSION, JevClassifier, PatternClassifier,
    RequestPattern, RequestPatternMode, RequestPatternSettings, current, install, parse_response,
    request_body, resolve, routes, settings,
};
use crate::cli::{Cli, RequestPatternArg};
use crate::config::{Action, Config};
use crate::minimal_loop::loop_run::RunSessionOptions;
use crate::minimal_loop::prompt::{
    PromptGuidance, ToolPromptMode, build_guided_request_messages, build_request_messages,
};
use crate::state::ConversationMessage;
use crate::tools::registry::ToolRegistry;

const REQUEST: &str = "Please repair the date parser in the existing module.";
const PROFILE: &str = "Profile block for the test.";

struct Scripted {
    answer: Result<Classification, ClassifyFailure>,
    calls: Cell<usize>,
}

impl Scripted {
    fn new(answer: Result<Classification, ClassifyFailure>) -> Self {
        Self {
            answer,
            calls: Cell::new(0),
        }
    }

    fn label(pattern: RequestPattern, confidence: f64) -> Self {
        Self::new(Ok(Classification {
            pattern: Some(pattern),
            confidence,
            model: Some("jev-1.14.0".to_string()),
        }))
    }
}

impl PatternClassifier for Scripted {
    fn classify(&self, request: &str) -> Result<Classification, ClassifyFailure> {
        assert_eq!(
            request, REQUEST,
            "the classifier receives the request text only"
        );
        self.calls.set(self.calls.get() + 1);
        self.answer.clone()
    }
}

fn jev() -> RequestPatternSettings {
    RequestPatternSettings {
        mode: RequestPatternMode::Jev,
        threshold: 0.8,
    }
}

fn system_prompt(guidance: PromptGuidance<'_>, mode: ToolPromptMode) -> String {
    build_guided_request_messages(
        &[ConversationMessage::user(REQUEST.to_string())],
        ToolRegistry::default().specs(),
        Path::new("/tmp/work"),
        Some("feedback kept"),
        guidance,
        mode,
    )[0]
    .content
    .clone()
}

fn prompt_config(root: &Path, extra: &[&str]) -> Config {
    let cwd = root.to_string_lossy().to_string();
    let mut args = vec!["commandagent", "--cwd", cwd.as_str()];
    args.extend_from_slice(extra);
    args.extend_from_slice(&["--prompt", REQUEST]);
    Config::from_cli(Cli::parse_from(args)).unwrap()
}

#[test]
fn default_options_build_the_unchanged_request_byte_for_byte() {
    let history = [ConversationMessage::user(REQUEST.to_string())];
    let options = RunSessionOptions::default();
    for mode in [ToolPromptMode::Native, ToolPromptMode::XmlFallback] {
        for profile in [None, Some(PROFILE)] {
            let plain = build_request_messages(
                &history,
                ToolRegistry::default().specs(),
                Path::new("/tmp/work"),
                Some("feedback kept"),
                profile,
                mode,
            );
            let guided = build_guided_request_messages(
                &history,
                ToolRegistry::default().specs(),
                Path::new("/tmp/work"),
                Some("feedback kept"),
                options.prompt_guidance(profile),
                mode,
            );
            assert_eq!(plain, guided);
            assert!(!guided[0].content.contains("Request guidance"));
        }
    }
}

#[test]
fn adopted_guidance_is_added_once_and_keeps_the_rest_of_the_request() {
    for pattern in RequestPattern::ALL {
        for mode in [ToolPromptMode::Native, ToolPromptMode::XmlFallback] {
            let prompt = system_prompt(
                PromptGuidance {
                    profile: Some(PROFILE),
                    request: Some(pattern.guidance()),
                },
                mode,
            );
            let base = system_prompt(
                PromptGuidance {
                    profile: Some(PROFILE),
                    request: None,
                },
                mode,
            );
            assert_eq!(prompt.matches("Request guidance:").count(), 1);
            assert_eq!(prompt.matches(pattern.guidance()).count(), 1);
            let profile_at = prompt.find("Profile guidance:\n").unwrap();
            let request_at = prompt.find("Request guidance:\n").unwrap();
            assert!(profile_at < request_at);
            if mode == ToolPromptMode::XmlFallback {
                assert!(request_at < prompt.find("Native tool calls are unavailable").unwrap());
            }
            let block = format!("\n\nRequest guidance:\n{}", pattern.guidance());
            assert_eq!(prompt.replacen(&block, "", 1), base);
        }
    }
    let messages = build_guided_request_messages(
        &[ConversationMessage::user(REQUEST.to_string())],
        ToolRegistry::default().specs(),
        Path::new("/tmp/work"),
        Some("feedback kept"),
        PromptGuidance {
            profile: None,
            request: Some(RequestPattern::Modify.guidance()),
        },
        ToolPromptMode::Native,
    );
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[1].content, REQUEST);
    assert_eq!(messages[2].content, "feedback kept");
    assert!(!messages[0].content.contains("Profile guidance"));
}

#[test]
fn guidance_v1_is_short_general_and_asks_create_and_modify_to_run_once() {
    assert_eq!(GUIDANCE_VERSION, "v1");
    let run_once = "at least once with a small input derived from the request; if it fails, fix it and run it again.";
    for pattern in RequestPattern::ALL {
        let text = pattern.guidance();
        let lines = text.lines().count();
        assert!((3..=6).contains(&lines), "{pattern:?} has {lines} lines");
        let lower = text.to_ascii_lowercase();
        for banned in [
            "jev", "gemma", "gpt", "qwen", "claude", "ollama", "next.js", "python", ".py", ".ts",
            ".js", ".rs", "bench",
        ] {
            assert!(!lower.contains(banned), "{pattern:?} names {banned}");
        }
        let runs_once =
            text.contains(run_once) && text.contains("Report only the results you observed.");
        assert_eq!(
            runs_once,
            matches!(pattern, RequestPattern::Create | RequestPattern::Modify),
            "{pattern:?}"
        );
    }
    assert!(
        RequestPattern::Answer
            .guidance()
            .contains("without creating or changing files")
    );
    assert!(
        RequestPattern::Investigate
            .guidance()
            .contains("Do not change the subject")
    );
    assert!(
        RequestPattern::Investigate
            .guidance()
            .contains("observations separately from hypotheses")
    );
}

#[test]
fn high_confidence_labels_are_adopted_at_or_above_the_threshold() {
    for pattern in RequestPattern::ALL {
        for confidence in [0.8, 0.95, 1.0] {
            let classifier = Scripted::label(pattern, confidence);
            let resolution = resolve(jev(), REQUEST, false, Some(&classifier));
            assert_eq!(resolution.pattern, Some(pattern));
            assert_eq!(resolution.reason, "adopted");
            assert_eq!(resolution.guidance(), Some(pattern.guidance()));
            assert_eq!(classifier.calls.get(), 1);
        }
    }
}

#[test]
fn every_failure_falls_back_to_default_without_guidance() {
    let cases: Vec<(Option<Scripted>, bool, &str)> = vec![
        (
            Some(Scripted::label(RequestPattern::Create, 0.79)),
            false,
            "below_threshold",
        ),
        (
            Some(Scripted::new(Err(ClassifyFailure::Http))),
            false,
            "http_error",
        ),
        (
            Some(Scripted::new(Err(ClassifyFailure::Timeout))),
            false,
            "timeout",
        ),
        (
            Some(Scripted::new(Err(ClassifyFailure::Parse))),
            false,
            "parse_error",
        ),
        (
            Some(Scripted::new(Ok(Classification {
                pattern: None,
                confidence: 0.99,
                model: None,
            }))),
            false,
            "unknown_label",
        ),
        (None, false, "missing_key"),
        (
            Some(Scripted::label(RequestPattern::Create, 1.0)),
            true,
            "offline",
        ),
    ];
    for (classifier, offline, reason) in cases {
        let resolution = resolve(
            jev(),
            REQUEST,
            offline,
            classifier.as_ref().map(|c| c as &dyn PatternClassifier),
        );
        assert_eq!(resolution.reason, reason);
        assert!(!resolution.adopted(), "{reason}");
        assert_eq!(resolution.guidance(), None, "{reason}");
        let event = resolution.event();
        assert_eq!(event["adopted"], false, "{reason}");
        assert_eq!(event["pattern"], "default", "{reason}");
        assert_eq!(event["reason"], reason);
        assert!(event["guidance_version"].is_null(), "{reason}");
        if offline {
            assert_eq!(
                classifier.unwrap().calls.get(),
                0,
                "offline never calls jev"
            );
        }
    }
}

#[test]
fn resolved_event_has_the_declared_fields_and_no_request_text() {
    let classifier = Scripted::label(RequestPattern::Modify, 0.97);
    let event = resolve(jev(), REQUEST, false, Some(&classifier)).event();
    let keys = event
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let mut expected = vec![
        "adopted",
        "confidence",
        "event",
        "guidance_version",
        "latency_ms",
        "model",
        "pattern",
        "reason",
        "source",
    ];
    expected.sort_unstable();
    assert_eq!(keys, expected);
    assert_eq!(event["event"], "request_pattern_resolved");
    assert_eq!(event["source"], "jev");
    assert_eq!(event["pattern"], "MODIFY");
    assert_eq!(event["confidence"], 0.97);
    assert_eq!(event["adopted"], true);
    assert_eq!(event["model"], "jev-1.14.0");
    assert_eq!(event["guidance_version"], "v1");
    let text = event.to_string();
    assert!(!text.contains(REQUEST));
    assert!(!text.contains("date parser"));
}

#[test]
fn request_body_carries_the_request_and_one_choice_question_only() {
    let body = request_body(REQUEST);
    let object = body.as_object().unwrap();
    assert_eq!(
        object.keys().map(String::as_str).collect::<Vec<_>>(),
        ["model", "questions", "state"]
    );
    assert_eq!(body["state"], REQUEST);
    assert_eq!(body["model"], "jev-latest");
    let questions = body["questions"].as_object().unwrap();
    assert_eq!(questions.len(), 1);
    let question = &questions["work"];
    assert_eq!(
        question
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["criteria", "instructions", "type"]
    );
    assert_eq!(question["type"], "choice");
    let mut labels = question["criteria"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    labels.sort_unstable();
    assert_eq!(labels, ["ANSWER", "CREATE", "INVESTIGATE", "MODIFY"]);
    let text = body.to_string().to_ascii_lowercase();
    for internal in [
        "commandagent",
        "minimal loop",
        "workspace",
        "jev_api_key",
        "bearer",
    ] {
        assert!(!text.contains(internal), "body mentions {internal}");
    }
}

#[test]
fn response_parsing_separates_unknown_labels_from_broken_answers() {
    let ok = parse_response(
        r#"{"model":"jev-1.14.0","answers":{"work":{"choice":"CREATE","confidence":0.91}}}"#,
    )
    .unwrap();
    assert_eq!(ok.pattern, Some(RequestPattern::Create));
    assert_eq!(ok.confidence, 0.91);
    assert_eq!(ok.model.as_deref(), Some("jev-1.14.0"));

    let unknown =
        parse_response(r#"{"answers":{"work":{"choice":"DELETE","confidence":0.99}}}"#).unwrap();
    assert_eq!(unknown.pattern, None);
    assert_eq!(unknown.model, None);

    let odd_model = parse_response(
        r#"{"model":"jev 1.14 <script>","answers":{"work":{"choice":"ANSWER","confidence":0.9}}}"#,
    )
    .unwrap();
    assert_eq!(odd_model.model, None);

    for broken in [
        "not json",
        "{}",
        r#"{"answers":{}}"#,
        r#"{"answers":{"work":{"choice":7,"confidence":0.9}}}"#,
        r#"{"answers":{"work":{"choice":"CREATE"}}}"#,
        r#"{"answers":{"work":{"choice":"CREATE","confidence":"high"}}}"#,
        r#"{"answers":{"work":{"choice":"CREATE","confidence":1.5}}}"#,
        r#"{"answers":{"work":{"choice":"CREATE","confidence":-0.1}}}"#,
    ] {
        assert_eq!(
            parse_response(broken),
            Err(ClassifyFailure::Parse),
            "{broken}"
        );
    }
}

#[test]
fn classifier_debug_output_hides_the_key() {
    let classifier = JevClassifier::new(
        "http://127.0.0.1:9",
        "secret-test-key-918",
        std::time::Duration::from_millis(10),
    );
    let debug = format!("{classifier:?}");
    assert!(!debug.contains("secret-test-key-918"));
    assert!(debug.contains("<redacted>"));
}

#[test]
fn install_scopes_settings_to_a_direct_prompt_and_always_restores() {
    assert_eq!(current(), RequestPatternSettings::OFF);
    {
        let _guard = install(&Action::Prompt(REQUEST.to_string()), jev());
        assert_eq!(current(), jev());
        {
            let _inner = install(&Action::Repl, jev());
            assert_eq!(current(), RequestPatternSettings::OFF);
        }
        assert_eq!(current(), jev());
    }
    assert_eq!(current(), RequestPatternSettings::OFF);

    fn failing_run() -> anyhow::Result<()> {
        let _guard = install(&Action::Prompt(REQUEST.to_string()), jev());
        assert_eq!(current(), jev());
        anyhow::bail!("run failed")
    }
    assert!(failing_run().is_err());
    assert_eq!(current(), RequestPatternSettings::OFF);

    let panicked = std::panic::catch_unwind(|| {
        let _guard = install(&Action::Prompt(REQUEST.to_string()), jev());
        panic!("run panicked");
    });
    assert!(panicked.is_err());
    assert_eq!(current(), RequestPatternSettings::OFF);
}

#[test]
fn plan_entries_and_the_tui_never_see_jev_settings() {
    for action in [
        Action::Repl,
        Action::PlanSteps(REQUEST.to_string()),
        Action::PlanRun(REQUEST.to_string()),
        Action::UltraPlan(REQUEST.to_string()),
        Action::UltraPlanRun(REQUEST.to_string()),
        Action::RunPlan(std::path::PathBuf::from("plan.yaml")),
        Action::RunUltraPlan(std::path::PathBuf::from("plan.yaml")),
    ] {
        let _guard = install(&action, jev());
        assert_eq!(current(), RequestPatternSettings::OFF, "{action:?}");
    }
}

#[test]
fn explicit_intent_and_off_mode_do_not_route() {
    let root = tempfile::tempdir().unwrap();
    let plain = prompt_config(root.path(), &[]);
    assert!(routes(jev(), &plain));
    assert!(!routes(RequestPatternSettings::OFF, &plain));
    for intent in ["create", "fix", "investigate"] {
        let explicit = prompt_config(root.path(), &["--intent", intent]);
        assert!(!routes(jev(), &explicit), "--intent {intent}");
    }
}

fn write_config(root: &Path, text: &str) {
    std::fs::create_dir_all(root.join(".commandagent")).unwrap();
    std::fs::write(root.join(".commandagent/config.toml"), text).unwrap();
}

#[test]
fn settings_default_to_off_and_read_the_flag_and_config_keys() {
    let root = tempfile::tempdir().unwrap();
    let defaults = settings(None, root.path()).unwrap();
    assert_eq!(defaults, RequestPatternSettings::OFF);
    assert_eq!(defaults.threshold, 0.8);

    write_config(
        root.path(),
        "request_pattern = \"jev\"\nrequest_pattern_threshold = 0.5\n",
    );
    let configured = settings(None, root.path()).unwrap();
    assert_eq!(configured.mode, RequestPatternMode::Jev);
    assert_eq!(configured.threshold, 0.5);
    let flagged = settings(Some(RequestPatternArg::Off), root.path()).unwrap();
    assert_eq!(flagged.mode, RequestPatternMode::Off);
    assert_eq!(flagged.threshold, 0.5);
    // The keys are accepted by the startup config validation.
    prompt_config(root.path(), &[]);

    for threshold in ["0", "1", "1.0", "0.0"] {
        write_config(
            root.path(),
            &format!("request_pattern_threshold = {threshold}\n"),
        );
        assert!(settings(Some(RequestPatternArg::Jev), root.path()).is_ok());
    }
}

#[test]
fn out_of_range_threshold_and_unknown_mode_are_configuration_errors() {
    let root = tempfile::tempdir().unwrap();
    for (line, key) in [
        (
            "request_pattern_threshold = 1.5",
            "request_pattern_threshold",
        ),
        (
            "request_pattern_threshold = -0.1",
            "request_pattern_threshold",
        ),
        (
            "request_pattern_threshold = nan",
            "request_pattern_threshold",
        ),
        (
            "request_pattern_threshold = \"0.8\"",
            "request_pattern_threshold",
        ),
        ("request_pattern = \"on\"", "request_pattern"),
        ("request_pattern = \"JEV\"", "request_pattern"),
        ("request_pattern = jev", "request_pattern"),
    ] {
        write_config(root.path(), &format!("{line}\n"));
        let error = settings(Some(RequestPatternArg::Jev), root.path())
            .expect_err(line)
            .to_string();
        assert!(error.contains(key), "{line}: {error}");
        assert!(error.contains(":1 "), "{line}: {error}");
    }
}

#[test]
fn request_pattern_flag_parses_off_and_jev_only() {
    let jev = Cli::try_parse_from(["commandagent", "--request-pattern", "jev"]).unwrap();
    let off = Cli::try_parse_from(["commandagent", "--request-pattern", "off"]).unwrap();
    let omitted = Cli::try_parse_from(["commandagent"]).unwrap();
    assert_eq!(jev.request_pattern, Some(RequestPatternArg::Jev));
    assert_eq!(off.request_pattern, Some(RequestPatternArg::Off));
    assert_eq!(omitted.request_pattern, None);
    assert!(Cli::try_parse_from(["commandagent", "--request-pattern", "auto"]).is_err());
}
