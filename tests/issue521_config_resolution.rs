//! Pins the provider/preset resolution of `Config::from_cli_with_provider_options`
//! (CLI > preset > config file > default, plus every field's source label and
//! the rejection messages) through the public entry points only, so the same
//! file runs unchanged before and after the resolution is moved to a leaf.
//!
//! Every case runs against a throwaway workspace and a throwaway `HOME`; the
//! caller's real home, config files and credentials are never read.

use std::fs;
use std::path::Path;
use std::sync::Mutex;

use clap::Parser;
use commandagent::cli::Cli;
use commandagent::config::{Config, ProviderCliOptions, ProviderRole};

static ENV_LOCK: Mutex<()> = Mutex::new(());

const FAKE_ENV_NAME: &str = "COMMANDAGENT_TEST_FAKE_BASE";
const SCRUBBED_ENV: [&str; 4] = [
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "LM_STUDIO_API_TOKEN",
    FAKE_ENV_NAME,
];

struct Case {
    name: &'static str,
    /// `(location, content)`; location is `ws:<rel>` or `home:<rel>`.
    files: &'static [(&'static str, &'static str)],
    args: &'static [&'static str],
    options: ProviderCliOptions,
    fake_env: Option<&'static str>,
    /// Substrings that must never appear in the outcome (secret values).
    forbid: &'static [&'static str],
    expect: &'static str,
}

fn options(
    executor: bool,
    planner: bool,
    base_url: Option<&str>,
    key: Option<&str>,
) -> ProviderCliOptions {
    ProviderCliOptions {
        executor_openai_compatible: executor,
        planner_openai_compatible: planner,
        base_url: base_url.map(str::to_string),
        api_key_env: key.map(str::to_string),
    }
}

fn snapshot(config: &Config) -> String {
    let s = &config.field_sources;
    let mut lines = vec![
        format!("model={:?} src={:?}", config.model, s.model),
        format!("provider={:?} src={:?}", config.provider, s.provider),
        format!(
            "planner_model={:?} src={:?}",
            config.planner_model, s.planner_model
        ),
        format!(
            "planner_provider={:?} src={:?}",
            config.planner_provider, s.planner_provider
        ),
        format!(
            "planner_think={:?} src={:?}",
            config.planner_think, s.planner_think
        ),
        format!("ollama_think={:?}", config.ollama_think),
        format!(
            "classifier_model={:?} src={:?}",
            config.classifier_model, s.classifier_model
        ),
        format!(
            "classifier_provider={:?} src={:?}",
            config.classifier_provider, s.classifier_provider
        ),
        format!(
            "context_budget={} src={:?}",
            config.context_budget, s.context_budget
        ),
        format!(
            "chat_timeout_secs={} src={:?} field_src={:?}",
            config.chat_timeout_secs, config.chat_timeout_source, s.chat_timeout_secs
        ),
        format!(
            "prompt_layout={:?} src={:?}",
            config.prompt_layout, s.prompt_layout
        ),
        format!(
            "plan_preset={:?} src={:?}",
            config.plan_preset, s.plan_preset
        ),
        format!("plan_preset_origin={}", config.plan_preset_origin()),
        format!(
            "profile={:?} explicit={} src={:?}",
            config.profile, config.profile_explicit, s.profile
        ),
        format!("narration={:?} src={:?}", config.narration, s.narration),
        format!("no_footer={} src={:?}", config.no_footer, s.footer),
        format!("stream={} src={:?}", config.stream, s.stream),
        format!("openai_api={:?}", config.openai_api),
        format!("tool_protocol={:?}", config.tool_protocol),
        format!("action={:?}", config.action),
    ];
    match &config.openai_compatible {
        Some(compatible) => lines.push(format!(
            "openai_compatible base_url={:?} api_key_env={:?} executor={} planner={} classifier={}",
            compatible.base_url,
            compatible.api_key_env,
            compatible.applies_to(ProviderRole::Executor),
            compatible.applies_to(ProviderRole::Planner),
            compatible.applies_to(ProviderRole::Classifier),
        )),
        None => lines.push("openai_compatible=none".to_string()),
    }
    lines.push(format!(
        "labels={}/{}/{}",
        config.provider_label(ProviderRole::Executor),
        config.provider_label(ProviderRole::Planner),
        config.provider_label(ProviderRole::Classifier)
    ));
    lines.join("\n")
}

fn scrub(text: &str, workspace: &Path, home: &Path) -> String {
    text.replace(workspace.to_str().unwrap(), "<WS>")
        .replace(home.to_str().unwrap(), "<HOME>")
}

fn run_case(case: &Case) -> String {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    let workspace = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let workspace_root = workspace.path().canonicalize().unwrap();
    let home_root = home.path().canonicalize().unwrap();
    for (location, content) in case.files {
        let path = if let Some(rel) = location.strip_prefix("ws:") {
            workspace_root.join(rel)
        } else {
            home_root.join(location.strip_prefix("home:").unwrap())
        };
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    // SAFETY: every env mutation in this test binary happens while holding
    // `ENV_LOCK`, so no other test thread reads or writes the environment.
    unsafe {
        for name in SCRUBBED_ENV {
            std::env::remove_var(name);
        }
        std::env::set_var("HOME", &home_root);
        if let Some(value) = case.fake_env {
            std::env::set_var(FAKE_ENV_NAME, value);
        }
    }
    let mut argv = vec!["commandagent", "--cwd", workspace_root.to_str().unwrap()];
    argv.extend_from_slice(case.args);
    let cli = Cli::try_parse_from(argv).expect("case arguments parse");
    let outcome = Config::from_cli_with_provider_options(cli, case.options.clone());
    // SAFETY: still under `ENV_LOCK`.
    unsafe {
        std::env::remove_var(FAKE_ENV_NAME);
    }
    match outcome {
        Ok(config) => format!(
            "OK\n{}",
            scrub(&snapshot(&config), &workspace_root, &home_root)
        ),
        Err(error) => format!(
            "ERR\n{}",
            scrub(&format!("{error:#}"), &workspace_root, &home_root)
        ),
    }
}

fn check(cases: &[Case]) {
    for case in cases {
        let actual = run_case(case);
        for secret in case.forbid {
            assert!(
                !actual.contains(secret),
                "case `{}` leaked a secret value",
                case.name
            );
        }
        assert_eq!(
            actual.trim_end(),
            case.expect.trim_end(),
            "case `{}` diverged from the pinned baseline",
            case.name
        );
    }
}

const FULL_PRESET: &str = r#"
[preset.full]
model = "preset-model"
provider = "lm-studio"
planner_model = "preset-planner"
planner_provider = "ollama"
planner_think = "low"
classifier_model = "preset-classifier"
classifier_provider = "gemini"
context_budget = 4096
chat_timeout_secs = 90
profile = "generic"
narration = "quiet"
footer = "off"
stream = "on"
prompt_layout = "stable"
plan_preset = "profile"
api = "responses"
tool_protocol = "text"
"#;

const ALL_FLAGS: &[&str] = &[
    "--model",
    "flag-model",
    "--provider",
    "gemini",
    "--planner-provider",
    "ollama",
    "--planner-model",
    "flag-planner",
    "--context-budget",
    "1234",
    "--chat-timeout-secs",
    "55",
    "--prompt-layout",
    "legacy",
    "--plan-preset",
    "none",
    "--stream",
    "off",
    "--footer",
    "on",
    "--profile",
    "generic",
    "--api",
    "chat-completions",
    "--tool-protocol",
    "native",
    "--think=medium",
];

fn no_options() -> ProviderCliOptions {
    ProviderCliOptions::default()
}

#[test]
fn cli_default_and_flag_precedence_without_preset() {
    check(&[
        Case {
            name: "all defaults",
            files: &[],
            args: &[],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "every flag set",
            files: &[],
            args: ALL_FLAGS,
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="flag-model" src="flag"
provider=Gemini src="flag"
planner_model="flag-planner" src="flag"
planner_provider=Ollama src="flag"
planner_think=Some(Medium) src="flag"
ollama_think=Some(Medium)
classifier_model="flag-planner" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=1234 src="flag"
chat_timeout_secs=55 src="flag" field_src="flag"
prompt_layout=Legacy src="flag"
plan_preset=None src="flag"
plan_preset_origin=cli
profile="generic" explicit=true src="flag"
narration=Normal src="default"
no_footer=false src="flag"
stream=false src="flag"
openai_api=ChatCompletions
tool_protocol=Some(Native)
action=Repl
openai_compatible=none
labels=gemini/ollama/ollama"#,
        },
        Case {
            name: "quiet and no-footer flags",
            files: &[],
            args: &["--quiet", "--no-footer"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Quiet src="flag"
no_footer=true src="flag"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "prompt action selects the non-interactive stream default",
            files: &[],
            args: &["--prompt", "hello"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=false src="default:non_interactive"
openai_api=ChatCompletions
tool_protocol=None
action=Prompt("hello")
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "planner model tier qwen27",
            files: &[],
            args: &["--planner-model", "qwen3.6:27b-x"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-x" src="flag"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-x" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "planner model tier gemma",
            files: &[],
            args: &["--planner-model", "gemma3"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="gemma3" src="flag"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="gemma3" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:gemma_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "remote providers use the remote chat timeout",
            files: &[],
            args: &["--provider", "gemini", "--planner-provider", "gemini"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Gemini src="flag"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Gemini src="flag"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Gemini src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=180 src="default:remote_provider" field_src="default:remote_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=gemini/gemini/gemini"#,
        },
        Case {
            name: "explicit data profile with fix intent",
            files: &[],
            args: &["--profile", "data", "--intent", "fix"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=Profile src="default_fix_data"
plan_preset_origin=default_fix_data
profile="data" explicit=true src="flag"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "explicit data profile with investigate intent",
            files: &[],
            args: &["--profile", "data", "--intent", "investigate"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=Profile src="default_investigate_data"
plan_preset_origin=default_investigate_data
profile="data" explicit=true src="flag"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "explicit ingest profile with create intent",
            files: &[],
            args: &["--profile", "ingest", "--intent", "create"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=Profile src="default_create_ingest"
plan_preset_origin=default_create_ingest
profile="ingest" explicit=true src="flag"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "ingest profile with fix intent has no profile default",
            files: &[],
            args: &["--profile", "ingest", "--intent", "fix"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="ingest" explicit=true src="flag"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "preset profile drives the profile plan-preset default",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.p]\nprofile = \"data\"\n",
            )],
            args: &["--preset", "p", "--intent", "fix"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=Profile src="default_fix_data"
plan_preset_origin=default_fix_data
profile="data" explicit=true src="preset:p"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "flag profile beats preset profile",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.p]\nprofile = \"data\"\n",
            )],
            args: &["--preset", "p", "--profile", "generic"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=true src="flag"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "pyproject workspace infers the python-cli profile",
            files: &[("ws:pyproject.toml", "[project]\nname = \"x\"\n")],
            args: &[],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="python-cli" explicit=false src="default:inferred"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "explicit profile suppresses workspace inference",
            files: &[("ws:pyproject.toml", "[project]\nname = \"x\"\n")],
            args: &["--profile", "generic"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=true src="flag"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "config-file plan preset beats the profile default",
            files: &[("ws:.commandagent/config.toml", "plan_preset = \"none\"\n")],
            args: &["--profile", "ingest", "--intent", "create"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="config:config.toml"
plan_preset_origin=config
profile="ingest" explicit=true src="flag"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "same provider planner inherits the executor model",
            files: &[],
            args: &["--model", "shared", "--provider", "lm-studio"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="shared" src="flag"
provider=LmStudio src="flag"
planner_model="shared" src="default"
planner_provider=LmStudio src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="shared" src="default:planner"
classifier_provider=LmStudio src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=lm-studio/lm-studio/lm-studio"#,
        },
    ]);
}

#[test]
fn preset_values_and_flag_over_preset() {
    check(&[
        Case {
            name: "preset supplies every field",
            files: &[("ws:.commandagent/config.toml", FULL_PRESET)],
            args: &["--preset", "full"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="preset-model" src="preset:full"
provider=LmStudio src="preset:full"
planner_model="preset-planner" src="preset:full"
planner_provider=Ollama src="preset:full"
planner_think=Some(Low) src="preset:full"
ollama_think=None
classifier_model="preset-classifier" src="preset:full"
classifier_provider=Gemini src="preset:full"
context_budget=4096 src="preset:full"
chat_timeout_secs=90 src="preset:full" field_src="preset:full"
prompt_layout=Stable src="preset:full"
plan_preset=Profile src="preset:full"
plan_preset_origin=config
profile="generic" explicit=true src="preset:full"
narration=Quiet src="preset:full"
no_footer=true src="preset:full"
stream=true src="preset:full"
openai_api=Responses
tool_protocol=Some(Text)
action=Repl
openai_compatible=none
labels=lm-studio/ollama/gemini"#,
        },
        Case {
            name: "flags override every preset field",
            files: &[("ws:.commandagent/config.toml", FULL_PRESET)],
            args: &[
                "--preset",
                "full",
                "--model",
                "flag-model",
                "--provider",
                "ollama",
                "--planner-provider",
                "ollama",
                "--planner-model",
                "flag-planner",
                "--context-budget",
                "1234",
                "--chat-timeout-secs",
                "55",
                "--prompt-layout",
                "legacy",
                "--plan-preset",
                "none",
                "--stream",
                "off",
                "--footer",
                "on",
                "--profile",
                "generic",
                "--api",
                "chat-completions",
                "--tool-protocol",
                "native",
                "--think=high",
            ],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="flag-model" src="flag"
provider=Ollama src="flag"
planner_model="flag-planner" src="flag"
planner_provider=Ollama src="flag"
planner_think=Some(High) src="flag"
ollama_think=Some(High)
classifier_model="preset-classifier" src="preset:full"
classifier_provider=Gemini src="preset:full"
context_budget=1234 src="flag"
chat_timeout_secs=55 src="flag" field_src="flag"
prompt_layout=Legacy src="flag"
plan_preset=None src="flag"
plan_preset_origin=cli
profile="generic" explicit=true src="flag"
narration=Quiet src="preset:full"
no_footer=false src="flag"
stream=false src="flag"
openai_api=ChatCompletions
tool_protocol=Some(Native)
action=Repl
openai_compatible=none
labels=ollama/ollama/gemini"#,
        },
        Case {
            name: "flag provider alone overrides the preset provider",
            files: &[("ws:.commandagent/config.toml", FULL_PRESET)],
            args: &["--preset", "full", "--provider", "gemini"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="preset-model" src="preset:full"
provider=Gemini src="flag"
planner_model="preset-planner" src="preset:full"
planner_provider=Ollama src="preset:full"
planner_think=Some(Low) src="preset:full"
ollama_think=None
classifier_model="preset-classifier" src="preset:full"
classifier_provider=Gemini src="preset:full"
context_budget=4096 src="preset:full"
chat_timeout_secs=90 src="preset:full" field_src="preset:full"
prompt_layout=Stable src="preset:full"
plan_preset=Profile src="preset:full"
plan_preset_origin=config
profile="generic" explicit=true src="preset:full"
narration=Quiet src="preset:full"
no_footer=true src="preset:full"
stream=true src="preset:full"
openai_api=Responses
tool_protocol=Some(Text)
action=Repl
openai_compatible=none
labels=gemini/ollama/gemini"#,
        },
        Case {
            name: "quiet flag overrides preset narration normal",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.p]\nnarration = \"normal\"\n",
            )],
            args: &["--preset", "p", "--quiet"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Quiet src="flag"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "no-footer flag overrides preset footer on",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.p]\nfooter = \"on\"\n",
            )],
            args: &["--preset", "p", "--no-footer"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=true src="flag"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "preset think is overridden by the flag and kept without it",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.p]\nplanner_think = \"high\"\n",
            )],
            args: &["--preset", "p"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(High) src="preset:p"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "blank preset name is ignored",
            files: &[],
            args: &["--preset", "  "],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
    ]);
}

#[test]
fn config_file_layers_and_inheritance() {
    check(&[
        Case {
            name: "earlier config file wins per preset field",
            files: &[
                (
                    "ws:.commandagent/config.toml",
                    "[preset.x]\nmodel = \"ws-commandagent\"\n",
                ),
                (
                    "ws:.anvil/config.toml",
                    "[preset.x]\nmodel = \"ws-anvil\"\nprovider = \"lm-studio\"\n",
                ),
                (
                    "home:.commandagent/config.toml",
                    "[preset.x]\nprovider = \"gemini\"\ncontext_budget = 111\n",
                ),
                (
                    "home:.anvil/config.toml",
                    "[preset.x]\ncontext_budget = 222\nchat_timeout_secs = 33\n",
                ),
            ],
            args: &["--preset", "x"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="ws-commandagent" src="preset:x"
provider=LmStudio src="preset:x"
planner_model="ws-commandagent" src="default"
planner_provider=LmStudio src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="ws-commandagent" src="default:planner"
classifier_provider=LmStudio src="default:planner"
context_budget=111 src="preset:x"
chat_timeout_secs=33 src="preset:x" field_src="preset:x"
prompt_layout=Legacy src="default"
plan_preset=None src="default"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=lm-studio/lm-studio/lm-studio"#,
        },
        Case {
            name: "child preset overrides and inherits from its parent",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.base]\nmodel = \"base-model\"\ncontext_budget = 777\nnarration = \"quiet\"\n\n[preset.child]\nextends = \"base\"\nmodel = \"child-model\"\n",
            )],
            args: &["--preset", "child"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="child-model" src="preset:child"
provider=Ollama src="default"
planner_model="child-model" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="child-model" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=777 src="preset:base"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Quiet src="preset:base"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "top-level keys layer under the preset and over the default",
            files: &[
                (
                    "ws:.commandagent/config.toml",
                    "narration = \"quiet\"\nfooter = \"off\"\nstream = \"on\"\nprompt_layout = \"stable\"\nplan_preset = \"profile\"\n",
                ),
                (
                    "home:.anvil/config.toml",
                    "narration = \"normal\"\nfooter = \"on\"\nstream = \"off\"\nprompt_layout = \"legacy\"\nplan_preset = \"none\"\n",
                ),
            ],
            args: &[],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Stable src="config:config.toml"
plan_preset=Profile src="config:config.toml"
plan_preset_origin=config
profile="generic" explicit=false src="default"
narration=Quiet src="config:config.toml"
no_footer=true src="config:config.toml"
stream=true src="config:config.toml"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "preset beats top-level keys",
            files: &[(
                "ws:.commandagent/config.toml",
                "narration = \"quiet\"\nfooter = \"off\"\nstream = \"on\"\nprompt_layout = \"stable\"\nplan_preset = \"profile\"\n\n[preset.p]\nnarration = \"normal\"\nfooter = \"on\"\nstream = \"off\"\nprompt_layout = \"legacy\"\nplan_preset = \"none\"\n",
            )],
            args: &["--preset", "p"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="preset:p"
plan_preset=None src="preset:p"
plan_preset_origin=config
profile="generic" explicit=false src="default"
narration=Normal src="preset:p"
no_footer=false src="preset:p"
stream=false src="preset:p"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
        Case {
            name: "flags beat top-level keys",
            files: &[(
                "ws:.commandagent/config.toml",
                "footer = \"off\"\nstream = \"on\"\nprompt_layout = \"stable\"\nplan_preset = \"profile\"\n",
            )],
            args: &[
                "--footer",
                "on",
                "--stream",
                "off",
                "--prompt-layout",
                "legacy",
                "--plan-preset",
                "none",
            ],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=Ollama src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="flag"
plan_preset=None src="flag"
plan_preset_origin=cli
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="flag"
stream=false src="flag"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=ollama/ollama/ollama"#,
        },
    ]);
}

#[test]
fn provider_and_model_rules() {
    check(&[
        Case {
            name: "planner provider differing without planner model",
            files: &[],
            args: &["--provider", "lm-studio", "--planner-provider", "ollama"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
--planner-model is required when --planner-provider differs from --provider"#,
        },
        Case {
            name: "classifier provider differing without classifier model",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.p]\nclassifier_provider = \"gemini\"\n",
            )],
            args: &["--preset", "p"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
classifier_model is required when classifier_provider differs from planner_provider"#,
        },
        Case {
            name: "think needs an ollama role",
            files: &[],
            args: &["--provider", "lm-studio", "--think=true"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
--think requires provider or planner_provider to be ollama"#,
        },
        Case {
            name: "think accepted when only the planner is ollama",
            files: &[],
            args: &[
                "--provider",
                "lm-studio",
                "--planner-provider",
                "ollama",
                "--planner-model",
                "pm",
                "--think=true",
            ],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=LmStudio src="flag"
planner_model="pm" src="flag"
planner_provider=Ollama src="flag"
planner_think=Some(True) src="flag"
ollama_think=Some(True)
classifier_model="pm" src="default:planner"
classifier_provider=Ollama src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=lm-studio/ollama/ollama"#,
        },
        Case {
            name: "openai executor rejects the ambiguous gpt-5.6 alias",
            files: &[],
            args: &["--provider", "openai", "--model", "gpt-5.6"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
OpenAI executor model alias `gpt-5.6` is ambiguous. Specify an exact Luna, Terra, or Sol model ID (`gpt-5.6-luna`, `gpt-5.6-terra`, or `gpt-5.6-sol`) or a provider-published snapshot-qualified ID."#,
        },
        Case {
            name: "openai planner rejects a malformed terra id",
            files: &[],
            args: &[
                "--planner-provider",
                "openai",
                "--planner-model",
                "gpt-5.6-terra-bad",
            ],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
OpenAI planner Terra model `gpt-5.6-terra-bad` is not an exact model ID or a date-qualified snapshot. Use `gpt-5.6-terra` or a provider-published `gpt-5.6-terra-YYYY-MM-DD` snapshot."#,
        },
        Case {
            name: "openai classifier from preset rejects the alias",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.p]\nclassifier_provider = \"openai\"\nclassifier_model = \"gpt-5.6\"\n",
            )],
            args: &["--preset", "p"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
OpenAI classifier model alias `gpt-5.6` is ambiguous. Specify an exact Luna, Terra, or Sol model ID (`gpt-5.6-luna`, `gpt-5.6-terra`, or `gpt-5.6-sol`) or a provider-published snapshot-qualified ID."#,
        },
        Case {
            name: "openai accepts an exact id",
            files: &[],
            args: &[
                "--provider",
                "openai",
                "--model",
                "gpt-5.6-sol",
                "--planner-provider",
                "openai",
            ],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="gpt-5.6-sol" src="flag"
provider=Openai src="flag"
planner_model="gpt-5.6-sol" src="default"
planner_provider=Openai src="flag"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="gpt-5.6-sol" src="default:planner"
classifier_provider=Openai src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=180 src="default:remote_provider" field_src="default:remote_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible=none
labels=openai/openai/openai"#,
        },
    ]);
}

#[test]
fn openai_compatible_resolution() {
    check(&[
        Case {
            name: "executor openai-compatible requires a base url",
            files: &[],
            args: &["--provider", "lm-studio"],
            options: options(true, false, None, None),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
openai-compatible requires --base-url or preset key base_url"#,
        },
        Case {
            name: "executor openai-compatible normalizes the base url",
            files: &[],
            args: &["--provider", "lm-studio"],
            options: options(
                true,
                false,
                Some("http://localhost:1234/v1/"),
                Some("VLLM_API_KEY"),
            ),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=LmStudio src="flag"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=LmStudio src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=LmStudio src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=180 src="default:remote_provider" field_src="default:remote_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible base_url="http://localhost:1234" api_key_env=Some("VLLM_API_KEY") executor=true planner=true classifier=true
labels=openai-compatible/openai-compatible/openai-compatible"#,
        },
        Case {
            name: "planner-only openai-compatible",
            files: &[],
            args: &["--planner-provider", "lm-studio", "--planner-model", "pm"],
            options: options(false, true, Some("https://example.invalid/api"), None),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=Ollama src="default"
planner_model="pm" src="flag"
planner_provider=LmStudio src="flag"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="pm" src="default:planner"
classifier_provider=LmStudio src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=600 src="default:local_provider" field_src="default:local_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible base_url="https://example.invalid/api" api_key_env=None executor=false planner=true classifier=true
labels=ollama/openai-compatible/openai-compatible"#,
        },
        Case {
            name: "base url without an openai-compatible role",
            files: &[],
            args: &[],
            options: options(false, false, Some("http://localhost:1234"), None),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
--base-url and --api-key-env require provider or planner_provider to be openai-compatible"#,
        },
        Case {
            name: "api key env without an openai-compatible role",
            files: &[],
            args: &[],
            options: options(false, false, None, Some("VLLM_API_KEY")),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
--base-url and --api-key-env require provider or planner_provider to be openai-compatible"#,
        },
        Case {
            name: "invalid api key env name",
            files: &[],
            args: &["--provider", "lm-studio"],
            options: options(true, false, Some("http://localhost:1234"), Some("bad name")),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
--api-key-env expects an environment variable name such as VLLM_API_KEY"#,
        },
        Case {
            name: "preset openai-compatible supplies base url and key env",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.c]\nprovider = \"openai-compatible\"\nbase_url = \"http://preset.invalid:8000/v1\"\napi_key_env = \"PRESET_KEY\"\n",
            )],
            args: &["--preset", "c"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=LmStudio src="preset:c"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=LmStudio src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=LmStudio src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=180 src="default:remote_provider" field_src="default:remote_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible base_url="http://preset.invalid:8000" api_key_env=Some("PRESET_KEY") executor=true planner=true classifier=true
labels=openai-compatible/openai-compatible/openai-compatible"#,
        },
        Case {
            name: "cli base url overrides the preset base url",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.c]\nprovider = \"openai-compatible\"\nbase_url = \"http://preset.invalid:8000/v1\"\napi_key_env = \"PRESET_KEY\"\n",
            )],
            args: &["--preset", "c"],
            options: options(
                false,
                false,
                Some("http://cli.invalid:9000"),
                Some("CLI_KEY"),
            ),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=LmStudio src="preset:c"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=LmStudio src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=LmStudio src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=180 src="default:remote_provider" field_src="default:remote_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible base_url="http://cli.invalid:9000" api_key_env=Some("CLI_KEY") executor=true planner=true classifier=true
labels=openai-compatible/openai-compatible/openai-compatible"#,
        },
        Case {
            name: "cli compat flag overrides a built-in preset provider",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.c]\nprovider = \"gemini\"\nbase_url = \"http://preset.invalid:8000\"\n",
            )],
            args: &["--preset", "c", "--provider", "lm-studio"],
            options: options(true, false, None, None),
            fake_env: None,
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=LmStudio src="flag"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=LmStudio src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=LmStudio src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=180 src="default:remote_provider" field_src="default:remote_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible base_url="http://preset.invalid:8000" api_key_env=None executor=true planner=true classifier=true
labels=openai-compatible/openai-compatible/openai-compatible"#,
        },
        Case {
            name: "env reference expands into the base url",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.e]\nprovider = \"openai-compatible\"\nbase_url = \"${COMMANDAGENT_TEST_FAKE_BASE}\"\n",
            )],
            args: &["--preset", "e"],
            options: no_options(),
            fake_env: Some("http://env.invalid:7000/v1"),
            forbid: &[],
            expect: r#"OK
model="qwen3.6:27b-coding-nvfp4" src="default"
provider=LmStudio src="preset:e"
planner_model="qwen3.6:27b-coding-nvfp4" src="default"
planner_provider=LmStudio src="default"
planner_think=Some(False) src="default"
ollama_think=None
classifier_model="qwen3.6:27b-coding-nvfp4" src="default:planner"
classifier_provider=LmStudio src="default:planner"
context_budget=65536 src="default"
chat_timeout_secs=180 src="default:remote_provider" field_src="default:remote_provider"
prompt_layout=Legacy src="default"
plan_preset=None src="default:qwen27_planner"
plan_preset_origin=default
profile="generic" explicit=false src="default"
narration=Normal src="default"
no_footer=false src="default"
stream=true src="default:repl"
openai_api=ChatCompletions
tool_protocol=None
action=Repl
openai_compatible base_url="http://env.invalid:7000" api_key_env=None executor=true planner=true classifier=true
labels=openai-compatible/openai-compatible/openai-compatible"#,
        },
        Case {
            name: "unset env reference is rejected",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.e]\nprovider = \"openai-compatible\"\nbase_url = \"${COMMANDAGENT_TEST_FAKE_BASE}\"\n",
            )],
            args: &["--preset", "e"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
<WS>/.commandagent/config.toml:3 preset.e.base_url: environment variable COMMANDAGENT_TEST_FAKE_BASE is not set or is not Unicode: environment variable not found"#,
        },
        Case {
            name: "credential-bearing env base url never echoes its value",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.e]\nprovider = \"openai-compatible\"\nbase_url = \"${COMMANDAGENT_TEST_FAKE_BASE}\"\n",
            )],
            args: &["--preset", "e"],
            options: no_options(),
            fake_env: Some("http://user:fake-password-9f3a1c@env.invalid/v1"),
            forbid: &["fake-password-9f3a1c"],
            expect: r#"ERR
--base-url must not contain credentials; use --api-key-env for server authentication"#,
        },
        Case {
            name: "unparseable env base url never echoes its value",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.e]\nprovider = \"openai-compatible\"\nbase_url = \"${COMMANDAGENT_TEST_FAKE_BASE}\"\n",
            )],
            args: &["--preset", "e"],
            options: no_options(),
            fake_env: Some("not a url fake-token-77d2e0"),
            forbid: &["fake-token-77d2e0"],
            expect: r#"ERR
invalid --base-url URL: relative URL without a base"#,
        },
    ]);
}

#[test]
fn preset_and_config_file_errors() {
    check(&[
        Case {
            name: "preset not found with no config files",
            files: &[],
            args: &["--preset", "nope"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
preset 'nope' not found in <WS>/.commandagent/config.toml, <WS>/.anvil/config.toml, <HOME>/.commandagent/config.toml, <HOME>/.anvil/config.toml (key: preset.nope)"#,
        },
        Case {
            name: "preset not found among existing files",
            files: &[
                (
                    "ws:.commandagent/config.toml",
                    "[preset.other]\nmodel = \"m\"\n",
                ),
                (
                    "home:.anvil/config.toml",
                    "[preset.another]\nmodel = \"m\"\n",
                ),
            ],
            args: &["--preset", "nope"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
preset 'nope' not found in <WS>/.commandagent/config.toml, <WS>/.anvil/config.toml, <HOME>/.commandagent/config.toml, <HOME>/.anvil/config.toml (key: preset.nope)"#,
        },
        Case {
            name: "preset inheritance cycle",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.a]\nextends = \"b\"\n\n[preset.b]\nextends = \"a\"\n",
            )],
            args: &["--preset", "a"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
preset 'a' extends 'b': preset 'b' extends 'a': preset inheritance cycle: a -> b -> a"#,
        },
        Case {
            name: "extends a missing parent",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.a]\nextends = \"ghost\"\n",
            )],
            args: &["--preset", "a"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
preset 'a' extends 'ghost': preset 'ghost' not found in <WS>/.commandagent/config.toml, <WS>/.anvil/config.toml, <HOME>/.commandagent/config.toml, <HOME>/.anvil/config.toml (key: preset.ghost)"#,
        },
        Case {
            name: "unknown preset key",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.p]\nbogus = \"x\"\n",
            )],
            args: &["--preset", "p"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
<WS>/.commandagent/config.toml:2 unknown config key 'preset.p.bogus'"#,
        },
        Case {
            name: "unknown top-level key is rejected by the preset-free path",
            files: &[("ws:.commandagent/config.toml", "bogus = \"x\"\n")],
            args: &[],
            options: no_options(),
            fake_env: None,
            forbid: &["\"x\""],
            expect: r#"ERR
<WS>/.commandagent/config.toml:1 unknown config key 'bogus'"#,
        },
        Case {
            name: "invalid preset provider value",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.p]\nprovider = \"mystery\"\n",
            )],
            args: &["--preset", "p"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
<WS>/.commandagent/config.toml:2 preset.p.provider expects provider ollama|lm-studio|openai|openai-compatible|gemini"#,
        },
        Case {
            name: "invalid preset planner think value",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.p]\nplanner_think = \"sometimes\"\n",
            )],
            args: &["--preset", "p"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
<WS>/.commandagent/config.toml:2 preset.p.planner_think expects think true|false|low|medium|high"#,
        },
        Case {
            name: "bare preset value is rejected as invalid TOML",
            files: &[("ws:.commandagent/config.toml", "[preset.p]\nmodel = bare\n")],
            args: &["--preset", "p"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
<WS>/.commandagent/config.toml:2 contains invalid TOML syntax"#,
        },
        Case {
            name: "non-numeric context budget in a preset",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.p]\ncontext_budget = \"many\"\n",
            )],
            args: &["--preset", "p"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
<WS>/.commandagent/config.toml:2 preset.p.context_budget expects an integer: invalid digit found in string"#,
        },
        Case {
            name: "invalid TOML syntax",
            files: &[("ws:.commandagent/config.toml", "[preset.p\nmodel = 1\n")],
            args: &["--preset", "p"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
<WS>/.commandagent/config.toml:1 contains invalid TOML syntax"#,
        },
        Case {
            name: "empty preset section header is rejected as invalid TOML",
            files: &[("ws:.commandagent/config.toml", "[preset.]\nmodel = \"m\"\n")],
            args: &["--preset", "p"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
<WS>/.commandagent/config.toml:1 contains invalid TOML syntax"#,
        },
        Case {
            name: "draft profile needs an extension root",
            files: &[],
            args: &["--profile", "mystery-draft"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
draft profile `mystery-draft` requires an extension root that declares profiles/mystery-draft/manifest.toml (use --extension-root or configure extension_root)"#,
        },
        Case {
            name: "preset narration with a bad value",
            files: &[(
                "ws:.commandagent/config.toml",
                "[preset.p]\nnarration = \"loud\"\n",
            )],
            args: &["--preset", "p"],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: r#"ERR
<WS>/.commandagent/config.toml:2 preset.p.narration expects narration normal|quiet"#,
        },
    ]);
}

const UNKNOWN_ERR: &str = "unknown config key 'bogus'";

fn expect_err_containing(case: &Case, needles: &[&str]) {
    let actual = run_case(case);
    assert!(
        actual.starts_with("ERR\n"),
        "case `{}` must fail at startup: {actual}",
        case.name
    );
    for needle in needles {
        assert!(
            actual.contains(needle),
            "case `{}` diagnostic lacks `{needle}`: {actual}",
            case.name
        );
    }
    for secret in case.forbid {
        assert!(
            !actual.contains(secret),
            "case `{}` leaked a value: {actual}",
            case.name
        );
    }
}

fn leak_files(files: Vec<(&'static str, &'static str)>) -> &'static [(&'static str, &'static str)] {
    Box::leak(files.into_boxed_slice())
}

#[test]
fn unknown_top_level_key_is_an_error_in_every_layer_and_mode() {
    let layers = [
        (
            "ws:.commandagent/config.toml",
            "<WS>/.commandagent/config.toml",
        ),
        ("ws:.anvil/config.toml", "<WS>/.anvil/config.toml"),
        (
            "home:.commandagent/config.toml",
            "<HOME>/.commandagent/config.toml",
        ),
        ("home:.anvil/config.toml", "<HOME>/.anvil/config.toml"),
    ];
    let invocations: [&[&str]; 4] = [
        &[],
        &["--preset", "ok"],
        &["--model", "flag-model"],
        &["--preset", "ok", "--model", "flag-model"],
    ];
    for (index, (location, display)) in layers.iter().enumerate() {
        // A valid preset lives in the next layer, so a selected preset would
        // resolve fine were the unknown key not rejected.
        let preset_location = layers[(index + 1) % layers.len()].0;
        for args in invocations {
            let case = Case {
                name: "unknown key in layer",
                files: leak_files(vec![
                    (location, "bogus = \"secret-value-1\"\n"),
                    (preset_location, "[preset.ok]\nmodel = \"m\"\n"),
                ]),
                args,
                options: no_options(),
                fake_env: None,
                forbid: &["secret-value-1"],
                expect: "",
            };
            expect_err_containing(&case, &[display, ":1 ", UNKNOWN_ERR]);
        }
    }
}

#[test]
fn broken_upper_layer_is_not_skipped_for_a_lower_layer() {
    // Unknown key above, valid footer below: the old behaviour applied the
    // lower footer; it must now be an error naming the upper file and line.
    expect_err_containing(
        &Case {
            name: "unknown key above a valid lower layer",
            files: &[
                ("ws:.commandagent/config.toml", "# c\nbogus = 1\n"),
                ("home:.commandagent/config.toml", "footer = \"off\"\n"),
            ],
            args: &[],
            options: no_options(),
            fake_env: None,
            forbid: &[],
            expect: "",
        },
        &["<WS>/.commandagent/config.toml:2 ", UNKNOWN_ERR],
    );
    // Broken lower layer below a valid upper layer is an error too.
    expect_err_containing(
        &Case {
            name: "invalid TOML below a valid upper layer",
            files: &[
                ("ws:.commandagent/config.toml", "footer = \"off\"\n"),
                ("home:.anvil/config.toml", "footer = oops-secret\n"),
            ],
            args: &[],
            options: no_options(),
            fake_env: None,
            forbid: &["oops-secret"],
            expect: "",
        },
        &["<HOME>/.anvil/config.toml:1 contains invalid TOML syntax"],
    );
}

#[test]
fn unreadable_config_path_is_an_error() {
    // A directory where `config.toml` should be a file cannot be read.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    let workspace = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let root = workspace.path().canonicalize().unwrap();
    fs::create_dir_all(root.join(".commandagent").join("config.toml")).unwrap();
    // SAFETY: env mutation is serialised by `ENV_LOCK`.
    unsafe {
        std::env::set_var("HOME", home.path());
    }
    let cli = Cli::try_parse_from(["commandagent", "--cwd", root.to_str().unwrap()]).unwrap();
    let error = Config::from_cli_with_provider_options(cli, no_options())
        .map(|_| ())
        .expect_err("an unreadable config layer must fail startup");
    assert!(format!("{error:#}").contains("failed to read"), "{error:#}");
}

#[test]
fn known_top_level_keys_unknown_sections_and_legacy_files_still_load() {
    let case = Case {
        name: "all known keys plus an unknown section",
        files: &[
            (
                "ws:.commandagent/config.toml",
                "narration = \"quiet\"\nfooter = \"off\"\nstream = \"off\"\nprompt_layout = \"stable\"\nplan_preset = \"none\"\n\n[other.section]\nanything = 1\n",
            ),
            // Extensionless legacy file keeps its lenient line scanner.
            (
                "ws:.anvil/config",
                "bogus line without equals\nunknown = 1\n",
            ),
        ],
        args: &[],
        options: no_options(),
        fake_env: None,
        forbid: &[],
        expect: "",
    };
    let actual = run_case(&case);
    assert!(actual.starts_with("OK\n"), "{actual}");
    assert!(
        actual.contains("narration=Quiet src=\"config:config.toml\""),
        "{actual}"
    );
    assert!(
        actual.contains("no_footer=true src=\"config:config.toml\""),
        "{actual}"
    );
    assert!(
        actual.contains("stream=false src=\"config:config.toml\""),
        "{actual}"
    );
    assert!(
        actual.contains("prompt_layout=Stable src=\"config:config.toml\""),
        "{actual}"
    );
    assert!(
        actual.contains("plan_preset=None src=\"config:config.toml\""),
        "{actual}"
    );
    // CLI flag still wins over the config file.
    let flagged = run_case(&Case {
        args: &["--footer", "on"],
        ..case
    });
    assert!(
        flagged.contains("no_footer=false src=\"flag\""),
        "{flagged}"
    );
}

#[test]
fn unselected_preset_with_unset_env_reference_does_not_block_startup() {
    const FILE: &str = "[preset.other]\nmodel = \"${COMMANDAGENT_TEST_UNSET_ENV_782}\"\n\n[preset.ok]\nmodel = \"m\"\n";
    let actual = run_case(&Case {
        name: "unselected preset references an unset env var",
        files: &[("ws:.commandagent/config.toml", FILE)],
        args: &[],
        options: no_options(),
        fake_env: None,
        forbid: &[],
        expect: "",
    });
    assert!(actual.starts_with("OK\n"), "{actual}");
    // Selecting that preset still reports the unset variable.
    let selected = run_case(&Case {
        name: "selected preset with an unset env var",
        files: &[("ws:.commandagent/config.toml", FILE)],
        args: &["--preset", "other"],
        options: no_options(),
        fake_env: None,
        forbid: &[],
        expect: "",
    });
    assert!(selected.starts_with("ERR\n"), "{selected}");
    assert!(
        selected.contains("COMMANDAGENT_TEST_UNSET_ENV_782"),
        "{selected}"
    );
    // Selecting a different preset starts and uses that preset's values.
    let other_selected = run_case(&Case {
        name: "another preset selected while one references an unset env var",
        files: &[("ws:.commandagent/config.toml", FILE)],
        args: &["--preset", "ok"],
        options: no_options(),
        fake_env: None,
        forbid: &[],
        expect: "",
    });
    assert!(other_selected.starts_with("OK\n"), "{other_selected}");
    assert!(other_selected.contains("model=\"m\""), "{other_selected}");
}

#[test]
fn unselected_preset_with_unknown_key_is_still_an_error() {
    const FILE: &str = "[preset.other]\nmodle = \"x\"\nmodel = \"${COMMANDAGENT_TEST_UNSET_ENV_782}\"\n\n[preset.ok]\nmodel = \"m\"\n";
    let actual = run_case(&Case {
        name: "unselected preset has an unknown key",
        files: &[("ws:.commandagent/config.toml", FILE)],
        args: &["--preset", "ok"],
        options: no_options(),
        fake_env: None,
        forbid: &[],
        expect: "",
    });
    assert!(actual.starts_with("ERR\n"), "{actual}");
    assert!(actual.contains("preset.other.modle"), "{actual}");
    assert!(
        !actual.contains("COMMANDAGENT_TEST_UNSET_ENV_782"),
        "{actual}"
    );
}
