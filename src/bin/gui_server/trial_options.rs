use axum::Json;
use commandagent::config::Provider;
use commandagent::planner::profile::ProfileId;
use commandagent::planner::profile_descriptor::descriptor;
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
    let profile = descriptor(&profile).expect("admitted profiles must have descriptors");
    ProfileOption {
        id: profile.canonical.to_string(),
        label: profile.display_name_ja.to_string(),
        description: profile.description_ja,
    }
}

const fn provider_model_hint(provider: Provider) -> &'static str {
    match provider {
        Provider::Ollama => "Ollama にインストール済みのモデルの正確な ID を入力してください。",
        Provider::LmStudio => {
            "LM Studio で読み込み済みモデルが公開する正確な ID を入力してください。"
        }
        Provider::Openai => {
            "サーバーの API キーで利用できる OpenAI モデルの正確な ID を入力してください。"
        }
        Provider::Gemini => {
            "サーバーの API キーで利用できる Gemini モデルの正確な ID を入力してください。"
        }
    }
}
