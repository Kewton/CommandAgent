use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use clap::{Parser, ValueEnum};

#[path = "gui_server/api.rs"]
mod api;
#[path = "gui_server/delegate.rs"]
mod delegate;
#[path = "gui_server/directives.rs"]
mod directives;
#[path = "gui_server/error_response.rs"]
mod error_response;
#[path = "gui_server/extensions.rs"]
mod extensions;
#[path = "gui_server/gate_one.rs"]
mod gate_one;
#[path = "gui_server/gui_contract.rs"]
mod gui_contract;
#[path = "gui_server/pack_catalog.rs"]
mod pack_catalog;
#[path = "gui_server/preflight.rs"]
mod preflight;
#[path = "gui_server/runtime_status.rs"]
mod runtime_status;
#[path = "gui_server/session_diagnostics.rs"]
mod session_diagnostics;
#[path = "gui_server/session_files.rs"]
mod session_files;
#[path = "gui_server/session_index.rs"]
mod session_index;
#[path = "gui_server/session_paths.rs"]
mod session_paths;
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
    pub ollama_host: String,
    pub lm_studio_host: String,
    pub extension_root: Option<PathBuf>,
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
    #[arg(long)]
    extension_root: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "off")]
    trial_token_auth: TrialTokenAuthArg,
    #[arg(long)]
    commandagent_bin: Option<PathBuf>,
    #[arg(
        long,
        default_value = "http://localhost:11434",
        help = "Set the Ollama server base URL used for model discovery and delegated runs."
    )]
    ollama_host: String,
    #[arg(
        long,
        default_value = "http://localhost:1234",
        help = "Set the LM Studio base URL used for model discovery and delegated runs."
    )]
    lm_studio_host: String,
    #[arg(long, conflicts_with = "check")]
    init: bool,
    #[arg(long)]
    check: bool,
    #[arg(long, requires = "check")]
    json: bool,
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
    let mut arguments = Arguments::parse();
    if arguments.init {
        initialize_defaults(&mut arguments)?;
    }
    let ollama_host = trial_options::normalize_model_host(
        &arguments.ollama_host,
        commandagent::config::Provider::Ollama,
    )?;
    let lm_studio_host = trial_options::normalize_model_host(
        &arguments.lm_studio_host,
        commandagent::config::Provider::LmStudio,
    )?;
    if arguments.check || arguments.init {
        let report = preflight::Report::run(&arguments);
        let passed = report.passed();
        if arguments.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            report.print_human();
        }
        if !passed {
            bail!("preflight checks failed");
        }
        if arguments.check {
            return Ok(());
        }
    }
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
    let extension_root = arguments
        .extension_root
        .as_deref()
        .map(|path| {
            commandagent::planner::pack::SupplyRoot::open(path)
                .map(|root| root.root().to_path_buf())
                .with_context(|| format!("open extension root {}", path.display()))
        })
        .transpose()?;
    if let Some(extension_root) = extension_root.as_deref() {
        workspace_policy::ensure_disjoint(&repository_root, extension_root)?;
        if let Some(execution_root) = trial_workspace.configured_path() {
            workspace_policy::ensure_disjoint(execution_root, extension_root)?;
        }
        commandagent::planner::extension_profiles::register(extension_root).with_context(|| {
            format!(
                "load draft profiles from extension root {}",
                extension_root.display()
            )
        })?;
    }
    let trial_access = trial_access::TrialAccess::from_environment(
        trial_workspace.is_enabled(),
        arguments.trial_token_auth.is_enabled(),
    )?;
    let commandagent_bin = resolve_commandagent_bin(&arguments, &repository_root);
    let execution_root_summary = trial_workspace
        .configured_path()
        .map_or_else(|| "-".to_string(), |path| path.display().to_string());
    let extension_root_summary = extension_root
        .as_deref()
        .map_or_else(|| "-".to_string(), |path| path.display().to_string());
    let approved_pack_count = preflight::count_packs(Some(&repository_root));
    let local_pack_count = preflight::count_packs(extension_root.as_deref());
    let state = AppState {
        repository_root,
        static_root: arguments.static_dir,
        base_path: base_path.clone(),
        commandagent_bin,
        ollama_host,
        lm_studio_host,
        extension_root: extension_root.clone(),
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
    println!(
        "gui_server auth={} execution_root={} extension_root={} packs={}/{} (approved/local)",
        if arguments.trial_token_auth.is_enabled() {
            "on"
        } else {
            "off"
        },
        execution_root_summary,
        extension_root_summary,
        approved_pack_count,
        local_pack_count
    );
    axum::serve(listener, app)
        .await
        .context("serve dashboard")?;
    Ok(())
}

fn initialize_defaults(arguments: &mut Arguments) -> anyhow::Result<()> {
    if arguments.execution_root.is_none() || arguments.extension_root.is_none() {
        let data_root = gui_data_root()?;
        if arguments.execution_root.is_none() {
            let execution_root = data_root.join("trial-workspace");
            prepare_private_root(&execution_root)?;
            arguments.execution_root = Some(execution_root);
        }
        if arguments.extension_root.is_none() {
            let extension_root = data_root.join("extensions");
            prepare_private_root(&extension_root)?;
            arguments.extension_root = Some(extension_root);
        }
    }
    if arguments.commandagent_bin.is_none() {
        let repository_root = arguments
            .repository_root
            .canonicalize()
            .unwrap_or_else(|_| arguments.repository_root.clone());
        arguments.commandagent_bin = discover_commandagent(&repository_root);
    }
    Ok(())
}

fn gui_data_root() -> anyhow::Result<PathBuf> {
    if let Some(root) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(root).join("commandagent"));
    }
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .context("HOME is unset; set HOME or XDG_DATA_HOME before using --init")?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("commandagent"))
}

fn prepare_private_root(root: &Path) -> anyhow::Result<()> {
    if std::fs::symlink_metadata(root).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        bail!(
            "refusing to initialize private GUI root through symlink {}",
            root.display()
        );
    }
    std::fs::create_dir_all(root)
        .with_context(|| format!("create private GUI root {}", root.display()))?;
    if !root.is_dir() {
        bail!("private GUI root is not a directory: {}", root.display());
    }
    set_private_permissions(root)?;
    Ok(())
}

#[cfg(unix)]
fn set_private_permissions(root: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("set private permissions on {}", root.display()))
}

#[cfg(not(unix))]
fn set_private_permissions(_: &Path) -> anyhow::Result<()> {
    Ok(())
}

fn discover_commandagent(repository_root: &Path) -> Option<PathBuf> {
    let executable = format!("commandagent{}", std::env::consts::EXE_SUFFIX);
    let beside_server = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(&executable)));
    let release = repository_root
        .join("target")
        .join("release")
        .join(&executable);
    beside_server
        .into_iter()
        .chain(std::iter::once(release))
        .chain(
            std::env::var_os("PATH")
                .into_iter()
                .flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
                .map(|directory| directory.join(&executable)),
        )
        .find(|candidate| candidate.is_file())
}

fn resolve_commandagent_bin(arguments: &Arguments, repository_root: &Path) -> PathBuf {
    let configured = arguments.commandagent_bin.as_deref().unwrap_or_else(|| {
        if arguments.init {
            Path::new("target/release/commandagent")
        } else {
            Path::new("target/debug/commandagent")
        }
    });
    if configured.is_absolute() {
        configured.to_path_buf()
    } else {
        repository_root.join(configured)
    }
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
        .route(
            "/api/provider-models",
            get(trial_options::get_provider_models),
        )
        .route("/api/band-means", get(api::band_means))
        .route("/api/pack-options", get(trial_options::get_packs))
        .route(
            "/api/extensions/packs",
            get(extensions::list)
                .post(extensions::stage)
                .layer(DefaultBodyLimit::max(extensions::MAX_BODY_BYTES)),
        )
        .route(
            "/api/extensions/packs/{id}/{version}",
            get(extensions::detail),
        )
        .route(
            "/api/extensions/packs/{id}/{version}/verify",
            post(extensions::verify),
        )
        .route(
            "/api/extensions/packs/{id}/{version}/pin",
            post(extensions::pin).layer(DefaultBodyLimit::max(extensions::MAX_BODY_BYTES)),
        )
        .route(
            "/api/extensions/packs/{id}/{version}/retire",
            post(extensions::retire),
        )
        .route("/api/runtime-status", get(runtime_status::get))
        .route("/api/session-proposals", post(gate_one::proposal))
        .route("/api/trial-workspace", get(sessions::workspace_status))
        .route(
            "/api/sessions",
            get(session_index::list).post(delegate::create),
        )
        .route("/api/sessions/{id}", get(sessions::status))
        .route(
            "/api/sessions/{id}/artifacts",
            get(session_files::artifacts),
        )
        .route("/api/sessions/{id}/events", get(session_files::events))
        .route("/api/sessions/{id}/directives", post(directives::propose))
        .route(
            "/api/sessions/{id}/directives/{hash}",
            post(directives::confirm),
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
