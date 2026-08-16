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
            "ブラウザー向け契約チェックを備えた Next.js App Router プロジェクト。",
        ),
        ProfileId::PythonCli => (
            "Python CLI",
            "使用方法と動作を検証する Python コマンドラインツール。",
        ),
        ProfileId::Data => (
            "表形式データパイプライン",
            "CSV または TSV の検査、変換、照合、レポート作成。",
        ),
        ProfileId::Ingest => (
            "スナップショット取り込みパイプライン",
            "ソースと候補件数を検証するオフライン・スナップショット抽出。",
        ),
        ProfileId::Generic => (
            "汎用",
            "許可済みの専用プロファイル契約を使用しない一般的な作業。",
        ),
        _ => (
            profile.as_str(),
            "許可済みの CommandAgent ランタイムプロファイル。",
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
