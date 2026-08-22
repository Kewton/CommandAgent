use std::collections::BTreeSet;
use std::time::Duration;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use commandagent::config::Provider;
use commandagent::planner::pack::catalog::{PackLocator, PackSource, admitted_packs};
use commandagent::planner::profile::ProfileId;
use commandagent::planner::profile_descriptor::descriptor;
use commandagent::tui::boundary_shell::route::admitted_profiles;
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::AppState;
use super::error_response::GuiError;

const ADMITTED_PROVIDERS: [Provider; 4] = [
    Provider::Ollama,
    Provider::LmStudio,
    Provider::Openai,
    Provider::Gemini,
];
const MODEL_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(2);

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
    status: &'static str,
    manifest_hash: Option<&'static str>,
    assurance_ceiling: &'static str,
    base_profile: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct ProviderOption {
    id: &'static str,
    label: &'static str,
    model_hint: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct ProviderModelsQuery {
    provider: Option<String>,
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

pub async fn get_provider_models(
    State(state): State<AppState>,
    Query(query): Query<ProviderModelsQuery>,
) -> Result<Json<Vec<String>>, GuiError> {
    let requested = query.provider.as_deref().unwrap_or("");
    let provider = local_provider(requested).ok_or_else(|| {
        GuiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "provider_models_unsupported",
            format!(
                "provider `{}` does not support local model discovery",
                requested
            ),
        )
    })?;
    let models = discover_models(&state, provider).await.unwrap_or_default();
    Ok(Json(models))
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

pub fn normalize_model_host(value: &str, provider: Provider) -> anyhow::Result<String> {
    let trimmed = value.trim().trim_end_matches('/');
    let normalized = if provider == Provider::LmStudio {
        trimmed.strip_suffix("/v1").unwrap_or(trimmed)
    } else {
        trimmed
    };
    if normalized.is_empty() {
        anyhow::bail!("--{}-host must not be empty", host_option_name(provider));
    }
    let parsed = reqwest::Url::parse(normalized).map_err(|error| {
        anyhow::anyhow!("invalid --{}-host URL: {error}", host_option_name(provider))
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        anyhow::bail!(
            "--{}-host must use http or https",
            host_option_name(provider)
        );
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        anyhow::bail!(
            "--{}-host must not contain credentials",
            host_option_name(provider)
        );
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        anyhow::bail!(
            "--{}-host must not contain a query or fragment",
            host_option_name(provider)
        );
    }
    Ok(normalized.to_string())
}

async fn discover_models(state: &AppState, provider: Provider) -> anyhow::Result<Vec<String>> {
    let client = reqwest::Client::builder()
        .connect_timeout(MODEL_DISCOVERY_TIMEOUT)
        .timeout(MODEL_DISCOVERY_TIMEOUT)
        .redirect(Policy::none())
        .build()?;
    let mut request = match provider {
        Provider::Ollama => client.get(format!("{}/api/tags", state.ollama_host)),
        Provider::LmStudio => client.get(format!("{}/v1/models", state.lm_studio_host)),
        Provider::Openai | Provider::Gemini => anyhow::bail!("provider is not local"),
    };
    if provider == Provider::LmStudio
        && let Some(token) = std::env::var_os("LM_STUDIO_API_TOKEN")
            .and_then(|value| value.into_string().ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    {
        request = request.bearer_auth(token);
    }
    let body = request
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    let entries = match provider {
        Provider::Ollama => body.get("models").and_then(Value::as_array),
        Provider::LmStudio => body.get("data").and_then(Value::as_array),
        Provider::Openai | Provider::Gemini => None,
    }
    .ok_or_else(|| anyhow::anyhow!("model list is missing"))?;
    let key = match provider {
        Provider::Ollama => "name",
        Provider::LmStudio => "id",
        Provider::Openai | Provider::Gemini => unreachable!("local provider checked above"),
    };
    Ok(entries
        .iter()
        .filter_map(|entry| entry.get(key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

fn local_provider(value: &str) -> Option<Provider> {
    match value {
        "ollama" => Some(Provider::Ollama),
        "lm-studio" => Some(Provider::LmStudio),
        _ => None,
    }
}

const fn host_option_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Ollama => "ollama",
        Provider::LmStudio => "lm-studio",
        Provider::Openai => "openai",
        Provider::Gemini => "gemini",
    }
}

fn options() -> TrialOptions {
    let mut profiles = admitted_profiles()
        .into_iter()
        .map(profile_option)
        .collect::<Vec<_>>();
    profiles.extend(
        commandagent::planner::extension_profiles::registered()
            .iter()
            .map(|extension| ProfileOption {
                id: extension.id.to_string(),
                label: extension.display_label.to_string(),
                description: extension.description,
                status: "draft",
                manifest_hash: Some(extension.manifest_hash),
                assurance_ceiling: extension.assurance_ceiling(),
                base_profile: extension.base_profile,
            }),
    );
    TrialOptions {
        profiles,
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
        status: "admitted",
        manifest_hash: None,
        assurance_ceiling: "full",
        base_profile: None,
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
