use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use anyhow::{Context, bail};
use axum::Router;
use axum::routing::{get, post};
use clap::Parser;

#[path = "gui_server/api.rs"]
mod api;
#[path = "gui_server/sessions.rs"]
mod sessions;
#[path = "gui_server/static_files.rs"]
mod static_files;

#[derive(Debug, Clone)]
pub struct AppState {
    pub repository_root: PathBuf,
    pub static_root: PathBuf,
    pub base_path: String,
    pub execution_root: PathBuf,
    pub commandagent_bin: PathBuf,
}

#[derive(Debug, Parser)]
#[command(about = "Serve the read-only CommandAgent dashboard")]
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
    #[arg(long, default_value = "target/debug/commandagent")]
    commandagent_bin: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = Arguments::parse();
    let base_path = normalize_base_path(&arguments.base_path)?;
    let execution_root = arguments
        .execution_root
        .unwrap_or_else(|| arguments.repository_root.clone());
    let commandagent_bin = if arguments.commandagent_bin.is_absolute() {
        arguments.commandagent_bin
    } else {
        arguments.repository_root.join(arguments.commandagent_bin)
    };
    let state = AppState {
        repository_root: arguments.repository_root,
        static_root: arguments.static_dir,
        base_path: base_path.clone(),
        execution_root,
        commandagent_bin,
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
        .route("/api/session-proposals", post(sessions::proposal))
        .route("/api/sessions", post(sessions::create))
        .route("/api/sessions/{id}", get(sessions::status))
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
