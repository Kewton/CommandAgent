use axum::Json;
use commandagent::config::Provider;
use commandagent::planner::profile::ProfileId;
use commandagent::tui::boundary_shell::route::admitted_profiles;
use serde::Serialize;

const ADMITTED_PROVIDERS: [Provider; 4] = [
    Provider::Ollama,
    Provider::LmStudio,
    Provider::Openai,
    Provider::Gemini,
];

#[derive(Debug, Serialize)]
pub struct TrialOptions {
    profiles: Vec<ProfileOption>,
    providers: Vec<ProviderOption>,
}

#[derive(Debug, Serialize)]
struct ProfileOption {
    id: String,
    label: String,
    description: &'static str,
}

#[derive(Debug, Serialize)]
struct ProviderOption {
    id: &'static str,
    label: &'static str,
    model_hint: &'static str,
}

pub async fn get() -> Json<TrialOptions> {
    Json(options())
}

pub fn is_admitted_provider(value: &str) -> bool {
    ADMITTED_PROVIDERS
        .iter()
        .any(|provider| provider.as_str() == value)
}

fn options() -> TrialOptions {
    TrialOptions {
        profiles: admitted_profiles()
            .into_iter()
            .map(profile_option)
            .collect(),
        providers: ADMITTED_PROVIDERS
            .into_iter()
            .map(|provider| ProviderOption {
                id: provider.as_str(),
                label: provider.display_name(),
                model_hint: provider_model_hint(provider),
            })
            .collect(),
    }
}

fn profile_option(profile: ProfileId) -> ProfileOption {
    let (label, description) = match &profile {
        ProfileId::Nextjs => (
            "Next.js",
            "Next.js App Router projects with browser-facing contract checks.",
        ),
        ProfileId::PythonCli => (
            "Python CLI",
            "Python command-line tools with usage and behavior checks.",
        ),
        ProfileId::Data => (
            "Tabular data pipeline",
            "CSV or TSV inspection, transformation, reconciliation, and reporting.",
        ),
        ProfileId::Ingest => (
            "Snapshot ingest pipeline",
            "Offline snapshot extraction with source and candidate-accounting checks.",
        ),
        ProfileId::Generic => (
            "Generic",
            "General work without a specialized admitted profile contract.",
        ),
        _ => (
            profile.as_str(),
            "An admitted CommandAgent runtime profile.",
        ),
    };
    ProfileOption {
        id: profile.to_string(),
        label: label.to_string(),
        description,
    }
}

const fn provider_model_hint(provider: Provider) -> &'static str {
    match provider {
        Provider::Ollama => "Use the exact ID of a model installed in Ollama.",
        Provider::LmStudio => {
            "Use the exact model identifier exposed by the loaded LM Studio model."
        }
        Provider::Openai => "Use an exact OpenAI model ID available to the server API key.",
        Provider::Gemini => "Use an exact Gemini model ID available to the server API key.",
    }
}
