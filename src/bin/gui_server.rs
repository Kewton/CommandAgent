use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use anyhow::{Context, bail};
use axum::Router;
use axum::routing::{get, post};
use clap::{Parser, ValueEnum};

#[path = "gui_server/api.rs"]
mod api;
#[path = "gui_server/error_response.rs"]
mod error_response;
#[path = "gui_server/runtime_status.rs"]
mod runtime_status;
#[path = "gui_server/session_files.rs"]
mod session_files;
#[path = "gui_server/session_index.rs"]
mod session_index;
#[path = "gui_server/sessions.rs"]
mod sessions;
#[path = "gui_server/static_files.rs"]
mod static_files;
#[path = "gui_server/trial_access.rs"]
mod trial_access;
#[path = "gui_server/trial_options.rs"]
mod trial_options;
#[path = "gui_server/workspace_policy.rs"]
mod workspace_policy;

#[derive(Debug, Clone)]
pub struct AppState {
    pub repository_root: PathBuf,
    pub static_root: PathBuf,
    pub base_path: String,
    pub commandagent_bin: PathBuf,
    pub trial_access: trial_access::TrialAccess,
    pub trial_workspace: workspace_policy::TrialWorkspace,
}

#[derive(Debug, Parser)]
#[command(about = "Serve the CommandAgent management dashboard")]
struct Arguments {
    #[arg(long, default_value_t = 4173)]
    port: u16,
    #[arg(long, default_value = "/")]
    base_path: String,
    #[arg(long, default_value = "gui/out")]
    static_dir: PathBuf,
    #[arg(long, default_value = ".")]
    repository_root: PathBuf,
    #[arg(long)]
    execution_root: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "off")]
    trial_token_auth: TrialTokenAuthArg,
    #[arg(long, default_value = "target/debug/commandagent")]
    commandagent_bin: PathBuf,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TrialTokenAuthArg {
    On,
    Off,
}

impl TrialTokenAuthArg {
    fn is_enabled(self) -> bool {
        matches!(self, Self::On)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = Arguments::parse();
    let base_path = normalize_base_path(&arguments.base_path)?;
    let repository_root = arguments.repository_root.canonicalize().with_context(|| {
        format!(
            "canonicalize repository root {}",
            arguments.repository_root.display()
        )
    })?;
    let trial_workspace = workspace_policy::TrialWorkspace::configure(
        &repository_root,
        arguments.execution_root.as_deref(),
    )?;
    let trial_access = trial_access::TrialAccess::from_environment(
        trial_workspace.is_enabled(),
        arguments.trial_token_auth.is_enabled(),
    )?;
    let commandagent_bin = if arguments.commandagent_bin.is_absolute() {
        arguments.commandagent_bin
    } else {
        repository_root.join(arguments.commandagent_bin)
    };
    let state = AppState {
        repository_root,
        static_root: arguments.static_dir,
        base_path: base_path.clone(),
        commandagent_bin,
        trial_access,
        trial_workspace,
    };
    let dashboard = dashboard_router();
    let app = if base_path == "/" {
        dashboard
    } else {
        Router::new()
            .route(&format!("{base_path}/"), get(static_files::serve))
            .nest(&base_path, dashboard)
    }
    .with_state(state);
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), arguments.port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("bind {address}"))?;
    let actual = listener.local_addr().context("read bound address")?;
    println!(
        "gui_server listening on http://{}:{}{}",
        actual.ip(),
        actual.port(),
        display_base_path(&base_path)
    );
    axum::serve(listener, app)
        .await
        .context("serve dashboard")?;
    Ok(())
}

fn dashboard_router() -> Router<AppState> {
    Router::new()
        .route("/", get(static_files::serve))
        .route("/api/runs", get(api::runs))
        .route("/api/runs/{id}", get(api::run_detail))
        .route("/api/runs/{id}/evidence", get(api::run_evidence))
        .route("/api/bands", get(api::bands))
        .route("/api/maps", get(api::maps))
        .route("/api/maps/score-time.svg", get(api::score_time_map))
        .route("/api/packs", get(api::packs))
        .route("/api/contracts", get(api::contracts))
        .route("/api/suites", get(api::suites))
        .route("/api/reports", get(api::reports))
        .route("/api/reports/view", get(api::report_content))
        .route("/api/trial-options", get(trial_options::get))
        .route("/api/runtime-status", get(runtime_status::get))
        .route("/api/session-proposals", post(sessions::proposal))
        .route("/api/trial-workspace", get(sessions::workspace_status))
        .route(
            "/api/sessions",
            get(session_index::list).post(sessions::create),
        )
        .route("/api/sessions/{id}", get(sessions::status))
        .route(
            "/api/sessions/{id}/artifacts",
            get(session_files::artifacts),
        )
        .route("/api/sessions/{id}/events", get(session_files::events))
        .route(
            "/api/sessions/{id}/directives",
            post(sessions::propose_directive),
        )
        .route(
            "/api/sessions/{id}/directives/{hash}",
            post(sessions::confirm_directive),
        )
        .fallback(static_files::serve)
}

fn normalize_base_path(value: &str) -> anyhow::Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return Ok("/".to_string());
    }
    if !trimmed.starts_with('/')
        || trimmed.ends_with('/')
        || trimmed.contains("//")
        || trimmed.contains("..")
        || trimmed.contains(['?', '#'])
    {
        bail!("--base-path must be '/' or an absolute path without a trailing slash")
    }
    Ok(trimmed.to_string())
}

fn display_base_path(base_path: &str) -> String {
    if base_path == "/" {
        "/".to_string()
    } else {
        format!("{base_path}/")
    }
}
