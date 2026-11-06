//! Which client a run under a wallet may have at all.
//!
//! Only the generic OpenAI-compatible Chat Completions route reports what a wallet needs to settle a
//! send, so every other route is refused before a client exists. A run that selected a wallet and
//! holds none is refused too, since a client built then would send with nothing counting it.
//!
//! With neither a selection nor a wallet, the only thing read here is the variable's presence.

use crate::config::{Config, OpenAiApi, ProviderRole};

/// Selects a wallet by its ID.
pub(crate) const RUN_BUDGET_ENV: &str = "COMMANDAGENT_RUN_BUDGET";

/// Fixed words only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdmissionRefusal {
    /// `COMMANDAGENT_RUN_BUDGET` is set, whatever its value, and this run holds no wallet.
    SelectedWithNoWallet,
    /// A wallet can only count the generic OpenAI-compatible Chat Completions route.
    RouteNotCountable,
}

impl std::fmt::Display for AdmissionRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "the run budget refused the client: {self:?}")
    }
}

impl std::error::Error for AdmissionRefusal {}

/// The decision, apart from where its three facts come from.
const fn decide(
    selected: bool,
    has_wallet: bool,
    countable_route: bool,
) -> Result<(), AdmissionRefusal> {
    match (selected, has_wallet, countable_route) {
        (false, false, _) => Ok(()),
        (true, false, _) => Err(AdmissionRefusal::SelectedWithNoWallet),
        (_, true, false) => Err(AdmissionRefusal::RouteNotCountable),
        (_, true, true) => Ok(()),
    }
}

fn countable_route(config: &Config, role: ProviderRole) -> bool {
    config.openai_api == OpenAiApi::ChatCompletions
        && config
            .openai_compatible
            .as_ref()
            .is_some_and(|compatible| compatible.applies_to(role))
}

/// Called before any client of a role is built.
pub(crate) fn admit_client(config: &Config, role: ProviderRole) -> Result<(), AdmissionRefusal> {
    decide(
        std::env::var_os(RUN_BUDGET_ENV).is_some(),
        config.field_sources.run_budget.is_some(),
        countable_route(config, role),
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use serde_json::json;

    use super::super::ledger::{CommandWrites, LedgerEntry, LedgerOpen};
    use super::super::policy::RunBudgetPolicy;
    use super::super::test_support::{FakeClock, WALLET, policy_value};
    use super::super::transport::RunBudgetHandle;
    use super::{AdmissionRefusal, RUN_BUDGET_ENV, decide};
    use crate::config::{Config, OpenAiApi, Provider, ProviderRole};
    use crate::providers::client_from_config_for_role;
    use crate::tools::os_sandbox::SandboxPolicy;

    #[test]
    fn the_decision_on_each_of_its_facts() {
        use AdmissionRefusal::{RouteNotCountable as Route, SelectedWithNoWallet as NoWallet};
        for (selected, wallet, route, expected) in [
            (false, false, false, Ok(())),
            (false, false, true, Ok(())),
            (true, false, false, Err(NoWallet)),
            (true, false, true, Err(NoWallet)),
            (false, true, false, Err(Route)),
            (false, true, true, Ok(())),
            (true, true, false, Err(Route)),
            (true, true, true, Ok(())),
        ] {
            assert_eq!(
                decide(selected, wallet, route),
                expected,
                "{selected} {wallet} {route}"
            );
        }
    }

    /// Nothing here runs a client: a refusal leaves none, and the URL is a discard address that a
    /// client built for the one control never asks.
    fn config(root: &Path, extra: &[&str]) -> Config {
        let (cwd, state) = (root.join("workspace"), root.join("state"));
        std::fs::create_dir_all(&cwd).unwrap();
        let mut arguments = vec![
            "commandagent".to_string(),
            "--cwd".to_string(),
            cwd.to_str().unwrap().to_string(),
            "--state-dir".to_string(),
            state.to_str().unwrap().to_string(),
            "--model".to_string(),
            "m-1".to_string(),
        ];
        arguments.extend(extra.iter().map(ToString::to_string));
        let parsed = crate::provider_cli::parse_from(arguments).unwrap();
        Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options).unwrap()
    }

    const COMPATIBLE: [&str; 4] = [
        "--provider",
        "openai-compatible",
        "--base-url",
        "http://127.0.0.1:9/v1",
    ];

    fn holding_a_wallet(config: &mut Config, root: &Path) {
        let mut value = policy_value();
        value["ledger_dir"] = json!(root.join("ledgers").to_str().unwrap());
        let policy = RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap();
        let workspace = config.workspace_root.clone();
        let run_tmp = root.join("run-tmp");
        std::fs::create_dir_all(&run_tmp).unwrap();
        let writes = SandboxPolicy::with_home(&workspace, &run_tmp, root.join("home"));
        let handle = RunBudgetHandle::open(LedgerOpen {
            policy: Arc::new(policy),
            workspace: &workspace,
            explicit_profile: None,
            entry: LedgerEntry::Fresh,
            clock: FakeClock::at(1),
            command_writes: Some(CommandWrites::of(writes)),
        })
        .unwrap();
        config.field_sources.run_budget = Some(Arc::new(handle));
    }

    fn refusal_of_the_factory(config: &Config, role: ProviderRole) -> Result<(), String> {
        client_from_config_for_role(config, role)
            .map(|_| ())
            .map_err(|error| format!("{error:#}"))
    }

    /// Every route the wallet cannot count, refused for what it is, by the factory itself. An error
    /// of another kind, such as a key that is not there, is not this refusal.
    #[test]
    fn a_run_with_a_wallet_gets_no_client_but_the_one_it_can_count() {
        const NOT_COUNTABLE: &str = "the run budget refused the client: RouteNotCountable";
        let mut wrong = Vec::new();
        type Change = fn(&mut Config);
        let rows: [(&str, Change); 6] = [
            ("ollama", |c| {
                c.openai_compatible = None;
                c.provider = Provider::Ollama;
            }),
            ("lm studio", |c| {
                c.openai_compatible = None;
                c.provider = Provider::LmStudio;
            }),
            ("openai", |c| {
                c.openai_compatible = None;
                c.provider = Provider::Openai;
            }),
            ("gemini", |c| {
                c.openai_compatible = None;
                c.provider = Provider::Gemini;
            }),
            ("responses", |c| c.openai_api = OpenAiApi::Responses),
            ("no change", |_| {}),
        ];
        for (name, change) in rows {
            let root = tempfile::tempdir().unwrap();
            let base = std::fs::canonicalize(root.path()).unwrap();
            let mut config = config(&base, &COMPATIBLE);
            holding_a_wallet(&mut config, &base);
            change(&mut config);
            let got = refusal_of_the_factory(&config, ProviderRole::Executor);
            let expected = if name == "no change" {
                Ok(())
            } else {
                Err(NOT_COUNTABLE.to_string())
            };
            if got != expected {
                wrong.push(format!("{name}: {got:?}"));
            }
        }

        // The endpoint is there and is not this role's, so the role would get a built-in client
        // that nothing counts. The last row is a role it is for.
        for (left_out, role, expected) in [
            (
                "planner_provider",
                ProviderRole::Planner,
                Err(NOT_COUNTABLE),
            ),
            (
                "classifier_provider",
                ProviderRole::Classifier,
                Err(NOT_COUNTABLE),
            ),
            ("provider", ProviderRole::Executor, Err(NOT_COUNTABLE)),
            (
                "planner_provider",
                ProviderRole::Executor,
                Ok("openai-compatible"),
            ),
        ] {
            let root = tempfile::tempdir().unwrap();
            let base = std::fs::canonicalize(root.path()).unwrap();
            let mut config = leaving_one_role_out(&base, left_out);
            holding_a_wallet(&mut config, &base);
            let applies = config
                .openai_compatible
                .as_ref()
                .is_some_and(|compatible| compatible.applies_to(role));
            let got = client_from_config_for_role(&config, role)
                .map(|client| client.label().to_string())
                .map_err(|error| format!("{error:#}"));
            let expected = expected.map(str::to_string).map_err(str::to_string);
            if got != expected || applies != expected.is_ok() {
                wrong.push(format!("{left_out} {role:?}: {got:?}, applies {applies}"));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
    }

    /// The endpoint for every role but `left_out`, which is Ollama's.
    fn leaving_one_role_out(root: &Path, left_out: &str) -> Config {
        let directory = root.join("workspace").join(".commandagent");
        std::fs::create_dir_all(&directory).unwrap();
        let mut preset = "[preset.leftout976]\nmodel = \"m-1\"\nplanner_model = \"m-1\"\nclassifier_model = \"m-1\"\nbase_url = \"http://127.0.0.1:9/v1\"\n".to_string();
        for key in ["provider", "planner_provider", "classifier_provider"] {
            let provider = if key == left_out {
                "ollama"
            } else {
                "openai-compatible"
            };
            preset.push_str(&format!("{key} = \"{provider}\"\n"));
        }
        std::fs::write(directory.join("config.toml"), preset).unwrap();
        config(root, &["--preset", "leftout976"])
    }

    const ENV_CHILD_ROOT: &str = "COMMANDAGENT_TEST_BUDGET_ADMISSION_ROOT";

    /// Runs only as the child of the test below, whose environment is all this process has.
    #[test]
    fn the_selection_with_no_wallet_child() {
        let Some(root) = std::env::var_os(ENV_CHILD_ROOT) else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let config = config(&root, &COMPATIBLE);
        let got = refusal_of_the_factory(&config, ProviderRole::Executor);
        if std::env::var_os(RUN_BUDGET_ENV).is_some() {
            assert_eq!(
                got,
                Err("the run budget refused the client: SelectedWithNoWallet".to_string())
            );
        } else {
            assert_eq!(got, Ok(()));
        }
    }

    /// One process with the variable and one without. Without it the factory makes its client as
    /// ever and finds no reason to touch a file; neither one sends, or touches a clock of ours.
    #[test]
    fn a_selection_with_no_wallet_gets_no_client_and_no_selection_is_as_it_was() {
        for selected in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let base = std::fs::canonicalize(root.path()).unwrap();
            let home = base.join("home");
            std::fs::create_dir_all(&home).unwrap();
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            child
                .args([
                    "--exact",
                    "structured_budget::admission::tests::the_selection_with_no_wallet_child",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env_clear()
                .env("HOME", &home)
                .env(ENV_CHILD_ROOT, &base);
            if selected {
                child.env(RUN_BUDGET_ENV, "wallet-selected-976");
            }
            let output = child.output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(output.status.success(), "{selected}\n{stdout}\n{stderr}");
            assert!(
                stdout.contains("1 passed"),
                "{selected}\n{stdout}\n{stderr}"
            );
            assert!(!base.join("ledgers").exists(), "{selected}");
        }
    }
}
