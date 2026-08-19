use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use commandagent::config::Provider;
use commandagent::planner::pack::catalog::{PackLocator, PackSource, admitted_packs};
use commandagent::planner::profile::ProfileId;
use commandagent::planner::profile_descriptor::descriptor;
use commandagent::tui::boundary_shell::route::admitted_profiles;
use serde::Serialize;

use super::AppState;
use super::error_response::GuiError;

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

#[derive(Debug, Serialize)]
pub struct PackOptions {
    packs: Vec<PackOption>,
}

#[derive(Debug, Serialize)]
struct PackOption {
    id: String,
    version: String,
    profile: String,
    intent: String,
    hash: String,
    point: String,
    source: PackSource,
    source_label: &'static str,
}

pub async fn get() -> Json<TrialOptions> {
    Json(options())
}

pub async fn get_packs(State(state): State<AppState>) -> Result<Json<PackOptions>, GuiError> {
    let repository_root = state.repository_root;
    let extension_root = state.extension_root;
    tokio::task::spawn_blocking(move || {
        let mut packs = admitted_packs()
            .iter()
            .map(|pack| PackOption {
                id: pack.id.to_string(),
                version: pack.version.to_string(),
                profile: pack.profile.to_string(),
                intent: pack.intent.to_string(),
                hash: pack.hash.to_string(),
                point: pack.point.to_string(),
                source: PackSource::Admitted,
                source_label: PackSource::Admitted.japanese_label(),
            })
            .collect::<Vec<_>>();
        if extension_root.is_some() {
            let locator = PackLocator::with_extension_root(repository_root, extension_root.clone());
            let root = commandagent::planner::pack::SupplyRoot::open(
                extension_root.as_deref().expect("checked extension root"),
            )
            .map_err(|error| error.to_string())?;
            for supplied in root.list().map_err(|error| error.to_string())? {
                if supplied.status != commandagent::planner::pack::catalog::PackStatus::Pinned
                    || !supplied.conformance_ok
                    || supplied.hash.as_ref() != supplied.pin.as_ref()
                {
                    continue;
                }
                let located = locator
                    .locate_pinned_from(PackSource::Local, &supplied.id, &supplied.version, None)
                    .map_err(|error| error.to_string())?;
                let Some(point) = located.point else {
                    continue;
                };
                packs.retain(|pack| pack.id != located.id || pack.version != located.version);
                packs.push(PackOption {
                    id: located.id,
                    version: located.version,
                    profile: located.profile,
                    intent: located.intent,
                    hash: located.hash,
                    point,
                    source: PackSource::Local,
                    source_label: PackSource::Local.japanese_label(),
                });
            }
        }
        packs.sort_by(|left, right| {
            (&left.profile, &left.intent, &left.id, &left.version).cmp(&(
                &right.profile,
                &right.intent,
                &right.id,
                &right.version,
            ))
        });
        Ok::<_, String>(Json(PackOptions { packs }))
    })
    .await
    .map_err(|error| {
        GuiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "extension_supply_failed",
            format!("join pack options task: {error}"),
        )
    })?
    .map_err(|error| {
        GuiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "extension_supply_failed",
            error,
        )
    })
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
