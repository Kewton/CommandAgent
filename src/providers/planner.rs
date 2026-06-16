use crate::config::{Config, Provider};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTarget {
    pub provider: Provider,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTargets {
    pub executor: ModelTarget,
    pub planner: ModelTarget,
}

pub fn resolve_targets(config: &Config) -> ProviderTargets {
    let executor = ModelTarget {
        provider: config.provider,
        model: config.model.clone(),
    };
    let planner = ModelTarget {
        provider: config.planner_provider.unwrap_or(config.provider),
        model: config
            .planner_model
            .clone()
            .or_else(|| config.model.clone()),
    };

    ProviderTargets { executor, planner }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn planner_defaults_to_executor_target() {
        let config = test_config(Provider::Ollama, Some("qwen".to_string()), None, None);
        let targets = resolve_targets(&config);

        assert_eq!(targets.executor.provider, Provider::Ollama);
        assert_eq!(targets.executor.model.as_deref(), Some("qwen"));
        assert_eq!(targets.planner.provider, Provider::Ollama);
        assert_eq!(targets.planner.model.as_deref(), Some("qwen"));
    }

    #[test]
    fn planner_can_use_different_provider_and_model() {
        let config = test_config(
            Provider::Ollama,
            Some("qwen-executor".to_string()),
            Some(Provider::Gemini),
            Some("gemini-planner".to_string()),
        );
        let targets = resolve_targets(&config);

        assert_eq!(targets.executor.provider, Provider::Ollama);
        assert_eq!(targets.executor.model.as_deref(), Some("qwen-executor"));
        assert_eq!(targets.planner.provider, Provider::Gemini);
        assert_eq!(targets.planner.model.as_deref(), Some("gemini-planner"));
    }

    fn test_config(
        provider: Provider,
        model: Option<String>,
        planner_provider: Option<Provider>,
        planner_model: Option<String>,
    ) -> Config {
        Config {
            cwd: PathBuf::from("/tmp/workspace"),
            state_dir: PathBuf::from("/tmp/workspace/.commandagent"),
            provider,
            planner_provider,
            model,
            planner_model,
            context_budget: 65536,
            max_iterations: 8,
            timeout_secs: 120,
            retries: 2,
            yes: false,
            offline: false,
            resume: None,
            gemini_api_key: None,
            openai_api_key: None,
        }
    }
}
