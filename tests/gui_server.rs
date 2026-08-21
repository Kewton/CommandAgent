use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use commandagent::planner::pack::catalog::ADMITTED_PACKS;
use commandagent::tui::boundary_shell::route::admitted_profiles;

const TEST_TRIAL_TOKEN: &str = "commandagent-gui-test-token-000000000001";

#[test]
fn gui_server_help_exposes_only_serving_inputs() {
    let output = Command::new(env!("CARGO_BIN_EXE_gui_server"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for option in [
        "--port",
        "--base-path",
        "--static-dir",
        "--repository-root",
        "--execution-root",
        "--extension-root",
        "--trial-token-auth",
        "--commandagent-bin",
        "--init",
        "--check",
        "--json",
    ] {
        assert!(help.contains(option), "missing {option}: {help}");
    }
    for forbidden in ["--provider", "--model", "mutation"] {
        assert!(!help.to_lowercase().contains(forbidden), "{help}");
    }
}

#[test]
fn gui_server_rejects_noncanonical_base_paths() {
    for value in ["proxy/gui", "/proxy/gui/", "/proxy//gui", "/../gui"] {
        let output = Command::new(env!("CARGO_BIN_EXE_gui_server"))
            .args(["--base-path", value])
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {value:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("--base-path"),
            "stderr for {value:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn gui_server_rejects_init_with_read_only_check_before_mutation() {
    let data_home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_gui_server"))
        .args(["--init", "--check"])
        .env("XDG_DATA_HOME", data_home.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cannot be used with '--check'"),
        "{}",
        combined_output(&output)
    );
    assert!(!data_home.path().join("commandagent").exists());
}

#[cfg(unix)]
#[test]
fn gui_server_check_reports_all_ok_without_binding() {
    use std::os::unix::fs::PermissionsExt;

    let repository = tempfile::tempdir().unwrap();
    let execution = tempfile::tempdir().unwrap();
    let extension = tempfile::tempdir().unwrap();
    std::fs::set_permissions(extension.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let static_root = repository.path().join("gui/out");
    std::fs::create_dir_all(&static_root).unwrap();
    std::fs::write(
        static_root.join("index.html"),
        r#"<script src="/proxy/commandagent/_next/static/app.js"></script>"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_gui_server"))
        .args(["--check", "--json", "--base-path", "/proxy/commandagent"])
        .arg("--repository-root")
        .arg(repository.path())
        .arg("--execution-root")
        .arg(execution.path())
        .arg("--extension-root")
        .arg(extension.path())
        .arg("--static-dir")
        .arg(&static_root)
        .arg("--commandagent-bin")
        .arg(env!("CARGO_BIN_EXE_commandagent"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", combined_output(&output));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "ok");
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|check| check["status"] == "ok")
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("listening on"));
}

#[test]
fn gui_server_check_reports_base_path_mismatch() {
    let repository = tempfile::tempdir().unwrap();
    let static_root = repository.path().join("out");
    std::fs::create_dir_all(&static_root).unwrap();
    std::fs::write(
        static_root.join("index.html"),
        r#"<script src="/built/path/_next/static/app.js"></script>"#,
    )
    .unwrap();
    let output = base_check_command(
        repository.path(),
        &static_root,
        Some(std::path::Path::new(env!("CARGO_BIN_EXE_commandagent"))),
    )
    .args(["--base-path", "/configured/path"])
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let text = combined_output(&output);
    assert!(text.contains("ng: static.base_path"), "{text}");
    assert!(
        text.contains(
            "fix: rebuild the export with `cd gui && GUI_BASE_PATH=/configured/path npm run build` so it matches --base-path"
        ),
        "{text}"
    );
}

#[test]
fn gui_server_check_reports_overlapping_roots() {
    let repository = tempfile::tempdir().unwrap();
    let static_root = write_root_export(repository.path());
    let execution = repository.path().join("execution");
    std::fs::create_dir_all(&execution).unwrap();
    let output = base_check_command(
        repository.path(),
        &static_root,
        Some(std::path::Path::new(env!("CARGO_BIN_EXE_commandagent"))),
    )
    .arg("--execution-root")
    .arg(&execution)
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let text = combined_output(&output);
    assert!(text.contains("ng: roots.disjoint"), "{text}");
    assert!(
        text.contains(
            "fix: move one of the named roots so repository, execution, and extension roots are pairwise disjoint"
        ),
        "{text}"
    );
}

#[test]
fn gui_server_check_reports_missing_binary() {
    let repository = tempfile::tempdir().unwrap();
    let static_root = write_root_export(repository.path());
    let output = base_check_command(repository.path(), &static_root, None)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let text = combined_output(&output);
    assert!(text.contains("ng: binary.version"), "{text}");
    assert!(
        text.contains("or pass its exact path with --commandagent-bin"),
        "{text}"
    );
}

#[test]
fn gui_server_check_reports_invalid_token() {
    let repository = tempfile::tempdir().unwrap();
    let static_root = write_root_export(repository.path());
    let output = base_check_command(
        repository.path(),
        &static_root,
        Some(std::path::Path::new(env!("CARGO_BIN_EXE_commandagent"))),
    )
    .args(["--trial-token-auth", "on"])
    .env("GUI_TRIAL_TOKEN", "too short")
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let text = combined_output(&output);
    assert!(text.contains("ng: trial.access"), "{text}");
    assert!(
        text.contains("fix: set GUI_TRIAL_TOKEN to 32..=4096 non-whitespace characters"),
        "{text}"
    );
}

#[test]
fn gui_server_check_reports_a_missing_static_export_with_its_build_command() {
    let repository = tempfile::tempdir().unwrap();
    let missing = repository.path().join("gui/out");
    let output = base_check_command(
        repository.path(),
        &missing,
        Some(std::path::Path::new(env!("CARGO_BIN_EXE_commandagent"))),
    )
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let text = combined_output(&output);
    assert!(
        text.contains(
            "fix: create the missing export with `cd gui && GUI_BASE_PATH=/ npm run build`"
        ),
        "{text}"
    );
}

#[cfg(unix)]
#[test]
fn gui_server_init_creates_private_default_roots_and_runs_preflight_before_listening() {
    use std::os::unix::fs::PermissionsExt;

    let repository = tempfile::tempdir().unwrap();
    let data_home = tempfile::tempdir().unwrap();
    let static_root = write_root_export(repository.path());
    let mut child = Command::new(env!("CARGO_BIN_EXE_gui_server"))
        .args(["--init", "--port", "0"])
        .arg("--repository-root")
        .arg(repository.path())
        .arg("--static-dir")
        .arg(&static_root)
        .env("XDG_DATA_HOME", data_home.path())
        .env_remove("GUI_TRIAL_TOKEN")
        .env_remove("GUI_TRIAL_ALLOWED_ORIGINS")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut startup = String::new();
    for _ in 0..20 {
        let mut line = String::new();
        if stdout.read_line(&mut line).unwrap() == 0 {
            let mut error = String::new();
            child
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut error)
                .unwrap();
            panic!("gui_server exited before listening: {startup}{error}");
        }
        startup.push_str(&line);
        if line.contains("gui_server listening on") {
            break;
        }
    }
    assert!(startup.contains("preflight: ok"), "{startup}");
    assert!(startup.contains("gui_server listening on"), "{startup}");
    assert!(startup.find("preflight: ok") < startup.find("gui_server listening on"));
    assert!(startup.contains("ok: binary.version"), "{startup}");

    let data_root = data_home.path().join("commandagent");
    for root in [
        data_root.join("trial-workspace"),
        data_root.join("extensions"),
    ] {
        assert!(root.is_dir(), "missing {}", root.display());
        assert_eq!(
            std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700,
            "{}",
            root.display()
        );
    }
    child.kill().unwrap();
    child.wait().unwrap();
}

#[cfg(unix)]
#[test]
fn gui_server_init_does_not_chmod_an_explicit_extension_root() {
    use std::os::unix::fs::PermissionsExt;

    let repository = tempfile::tempdir().unwrap();
    let data_home = tempfile::tempdir().unwrap();
    let extension = tempfile::tempdir().unwrap();
    let static_root = write_root_export(repository.path());
    std::fs::set_permissions(extension.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_gui_server"))
        .arg("--init")
        .arg("--repository-root")
        .arg(repository.path())
        .arg("--static-dir")
        .arg(&static_root)
        .arg("--extension-root")
        .arg(extension.path())
        .arg("--commandagent-bin")
        .arg(env!("CARGO_BIN_EXE_commandagent"))
        .env("XDG_DATA_HOME", data_home.path())
        .env_remove("GUI_TRIAL_TOKEN")
        .env_remove("GUI_TRIAL_ALLOWED_ORIGINS")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        std::fs::metadata(extension.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    let text = combined_output(&output);
    assert!(
        text.contains("permissions are 755; group/other permissions must be removed"),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "fix: remove group/other access with `chmod 700 {}`",
            extension.path().canonicalize().unwrap().display()
        )),
        "{text}"
    );
}

fn base_check_command(
    repository: &std::path::Path,
    static_root: &std::path::Path,
    binary: Option<&std::path::Path>,
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gui_server"));
    command
        .arg("--check")
        .arg("--repository-root")
        .arg(repository)
        .arg("--static-dir")
        .arg(static_root)
        .env_remove("GUI_TRIAL_TOKEN")
        .env_remove("GUI_TRIAL_ALLOWED_ORIGINS");
    if let Some(binary) = binary {
        command.arg("--commandagent-bin").arg(binary);
    }
    command
}

fn write_root_export(repository: &std::path::Path) -> std::path::PathBuf {
    let static_root = repository.join("out");
    std::fs::create_dir_all(&static_root).unwrap();
    std::fs::write(
        static_root.join("index.html"),
        r#"<script src="/_next/static/app.js"></script>"#,
    )
    .unwrap();
    static_root
}

fn combined_output(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn gui_server_disables_trial_without_an_execution_root() {
    let mut server = Server::start_dashboard_only();
    let dashboard = server.request_without_access("GET", "/", None);
    assert_eq!(dashboard.status, 200, "{}", dashboard.body);
    let runtime = server.request_without_access("GET", "/api/runtime-status", None);
    assert_eq!(runtime.status, 200, "{}", runtime.body);
    let runtime: serde_json::Value = serde_json::from_str(&runtime.body).unwrap();
    assert_eq!(runtime["trial_available"], false);
    assert_eq!(runtime["trial_token_auth_enabled"], false);
    assert!(runtime["session"].is_null());
    assert_eq!(
        runtime["prerequisites"]["execution_root"]["status"],
        "unconfigured"
    );
    assert_eq!(
        runtime["prerequisites"]["trial_authentication"]["status"],
        "ready"
    );
    let response =
        server.request_without_access("POST", "/api/session-proposals", Some(&session_spec()));
    assert_eq!(response.status, 503, "{}", response.body);
    assert_error(
        &response,
        "trial_execution_disabled",
        "trial execution is disabled; configure --execution-root",
    );
    server.stop();
}

#[test]
fn gui_server_read_errors_use_the_shared_coded_json_contract() {
    let mut server = Server::start_dashboard_only();
    let response =
        server.request_without_access("GET", "/api/runs/issue-72-record-that-does-not-exist", None);
    assert_eq!(response.status, 404, "{}", response.body);
    let payload: serde_json::Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(payload["code"], "resource_not_found");
    assert!(
        payload["error"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    server.stop();
}

#[cfg(unix)]
#[test]
fn gui_server_caches_hashed_next_assets_but_not_html() {
    let temp = tempfile::tempdir().unwrap();
    let static_root = temp.path().join("out");
    let chunk = static_root.join("_next/static/chunks/app-deadbeef.js");
    let nested = static_root.join("other/_next/static/not-content-addressed.js");
    std::fs::create_dir_all(chunk.parent().unwrap()).unwrap();
    std::fs::create_dir_all(nested.parent().unwrap()).unwrap();
    std::fs::write(
        static_root.join("index.html"),
        "<!doctype html><title>GUI</title>",
    )
    .unwrap();
    std::fs::write(&chunk, "console.log('immutable');").unwrap();
    std::fs::write(&nested, "console.log('not immutable');").unwrap();
    let mut server = Server::start_dashboard_only_with_static_root(&static_root);

    let index = server.request_without_access("GET", "/", None);
    assert_eq!(index.status, 200, "{}", index.body);
    assert_eq!(index.header("cache-control"), Some("no-store"));

    let asset = server.request_without_access("GET", "/_next/static/chunks/app-deadbeef.js", None);
    assert_eq!(asset.status, 200, "{}", asset.body);
    assert_eq!(
        asset.header("cache-control"),
        Some("public, max-age=31536000, immutable")
    );

    let nested =
        server.request_without_access("GET", "/other/_next/static/not-content-addressed.js", None);
    assert_eq!(nested.status, 200, "{}", nested.body);
    assert_eq!(nested.header("cache-control"), Some("no-store"));
    server.stop();
}

#[cfg(unix)]
#[test]
fn gui_server_redirects_exported_routes_and_serves_the_404_page_at_both_base_paths() {
    let temp = tempfile::tempdir().unwrap();
    let static_root = temp.path().join("out");
    std::fs::create_dir_all(static_root.join("try")).unwrap();
    std::fs::write(
        static_root.join("try/index.html"),
        "<!doctype html><title>Try</title>",
    )
    .unwrap();
    std::fs::write(
        static_root.join("404.html"),
        "<!doctype html><title>Not found</title>",
    )
    .unwrap();

    for base_path in ["/", "/proxy/commandagent"] {
        let prefix = base_path.trim_end_matches('/');
        let mut server =
            Server::start_dashboard_only_with_static_root_at_base_path(&static_root, base_path);

        let redirect = server.request_without_access("GET", &format!("{prefix}/try"), None);
        assert_eq!(redirect.status, 308, "{}", redirect.body);
        assert_eq!(
            redirect.header("location"),
            Some(format!("{prefix}/try/").as_str())
        );

        let query_redirect =
            server.request_without_access("GET", &format!("{prefix}/try?view=compact"), None);
        assert_eq!(query_redirect.status, 308, "{}", query_redirect.body);
        assert_eq!(
            query_redirect.header("location"),
            Some(format!("{prefix}/try/?view=compact").as_str())
        );

        let index = server.request_without_access("GET", &format!("{prefix}/try/"), None);
        assert_eq!(index.status, 200, "{}", index.body);
        assert!(index.body.contains("<title>Try</title>"));

        let missing = server.request_without_access("GET", &format!("{prefix}/nope/"), None);
        assert_eq!(missing.status, 404, "{}", missing.body);
        assert_eq!(
            missing.header("content-type"),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(missing.header("cache-control"), Some("no-store"));
        assert!(missing.body.contains("<title>Not found</title>"));
        server.stop();
    }
}

#[cfg(unix)]
#[test]
fn trial_options_match_admitted_profiles_without_trial_access() {
    let mut server = Server::start_dashboard_only();
    let response = server.request_without_access("GET", "/api/trial-options", None);
    assert_eq!(response.status, 200, "{}", response.body);
    let body: serde_json::Value = serde_json::from_str(&response.body).unwrap();
    let actual_profiles = body["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|option| option["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    let expected_profiles = admitted_profiles()
        .into_iter()
        .map(|profile| profile.to_string())
        .collect::<Vec<_>>();
    assert_eq!(actual_profiles, expected_profiles);
    assert!(body["profiles"].as_array().unwrap().iter().all(|option| {
        option["label"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
            && option["description"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
            && option["status"] == "admitted"
            && option["manifest_hash"].is_null()
    }));

    let providers = body["providers"].as_array().unwrap();
    assert_eq!(
        providers
            .iter()
            .map(|option| option["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["ollama", "lm-studio", "openai", "gemini"]
    );
    assert!(providers.iter().all(|option| {
        option["label"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
            && option["model_hint"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
    }));

    let response = server.request_without_access("GET", "/api/pack-options", None);
    assert_eq!(response.status, 200, "{}", response.body);
    let packs = response.json()["packs"].as_array().unwrap().clone();
    let cli_packs = packs
        .iter()
        .filter(|pack| pack["profile"] == "python-cli" && pack["intent"] == "create")
        .collect::<Vec<_>>();
    assert_eq!(
        cli_packs
            .iter()
            .map(|pack| format!(
                "{}@{}",
                pack["id"].as_str().unwrap(),
                pack["version"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        ["cli-assist@1.0.0", "cli-assist@1.1.0"]
    );
    assert!(cli_packs.iter().all(|pack| {
        pack["source"] == "admitted"
            && pack["source_label"] == "承認済み"
            && pack["hash"]
                .as_str()
                .is_some_and(|hash| hash.starts_with("sha256:"))
            && pack["point"] == "cli-validation"
    }));
    server.stop();
}

#[cfg(unix)]
#[test]
fn gui_lists_and_proposes_an_external_draft_profile_without_a_pack() {
    use std::os::unix::fs::PermissionsExt;

    let extension = tempfile::tempdir().unwrap();
    std::fs::set_permissions(extension.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let profile_dir = extension.path().join("profiles/static-site");
    std::fs::create_dir_all(&profile_dir).unwrap();
    std::fs::write(
        profile_dir.join("manifest.toml"),
        include_str!(
            "corpus/apps/issue117-draft-profile/extension-root/profiles/static-site/manifest.toml"
        ),
    )
    .unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let static_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("gui/out");
    let mut server = Server::start_with_repository_root_and_env(
        Some(workspace.path()),
        std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
        false,
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        StaticExport::new(&static_root, "/"),
        Some(extension.path()),
        &[],
    );

    let response = server.request_without_access("GET", "/api/trial-options", None);
    assert_eq!(response.status, 200, "{}", response.body);
    let options: serde_json::Value = serde_json::from_str(&response.body).unwrap();
    let draft = options["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|profile| profile["id"] == "static-site")
        .unwrap();
    assert_eq!(draft["status"], "draft");
    assert_eq!(draft["assurance_ceiling"], "static");
    assert!(draft["label"].as_str().unwrap().contains("下書き"));
    assert!(
        draft["manifest_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );

    let spec = serde_json::json!({
        "goal": "Create the requested static site",
        "profile": "static-site",
        "provider": "ollama",
        "model": "fixture-executor",
        "planner_provider": "ollama",
        "planner_model": "fixture-planner",
        "pack": null
    });
    let origin = format!("http://127.0.0.1:{}", server.port);
    let response = server.request_with_access(
        "POST",
        "/api/session-proposals",
        Some(&spec),
        None,
        Some(&origin),
    );
    assert_eq!(response.status, 200, "{}", response.body);
    let proposal: serde_json::Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(proposal["identity"]["profile"], "static-site");
    assert_eq!(proposal["identity"]["pack"]["selection"], "none");
    assert_eq!(
        proposal["identity"]["draft_manifest"]["assurance_ceiling"],
        "static"
    );
    assert!(
        proposal["card_markdown"]
            .as_str()
            .unwrap()
            .contains("draft / 未承認 / 保証上限 static")
    );
    assert_eq!(proposal["price"]["source"], "未計測");

    let mut packed = spec;
    packed["pack"] = serde_json::Value::String("nextjs-quality@1.0.0".to_string());
    let response = server.request_with_access(
        "POST",
        "/api/session-proposals",
        Some(&packed),
        None,
        Some(&origin),
    );
    assert_eq!(response.status, 422, "{}", response.body);
    assert!(response.body.contains("pack selection to none"));
    server.stop();
}

#[cfg(unix)]
#[test]
fn extension_catalog_classifies_supply_and_warns_on_stale_local_pins() {
    use std::os::unix::fs::PermissionsExt;

    let mut repository_server = Server::start_dashboard_only();
    let response = repository_server.request_without_access("GET", "/api/packs", None);
    assert_eq!(response.status, 200, "{}", response.body);
    let packs: Vec<serde_json::Value> = serde_json::from_str(&response.body).unwrap();
    assert_eq!(
        packs
            .iter()
            .filter(|pack| pack["source"] == "admitted")
            .count(),
        ADMITTED_PACKS.len()
    );
    assert!(packs.iter().all(|pack| {
        pack["source"] != "admitted"
            || (pack["source_label"] == "承認済み"
                && pack["hash_matches_pin"] == true
                && pack["pin"] == pack["expected_hash"]
                && pack["trial_eligible"] == true)
    }));
    let nextjs = packs
        .iter()
        .find(|pack| pack["id"] == "nextjs-acme")
        .expect("missing unadmitted repository pack");
    assert_eq!(nextjs["source"], "repository");
    assert_eq!(nextjs["source_label"], "リポジトリ（未承認）");
    repository_server.stop();

    let extension = tempfile::tempdir().unwrap();
    std::fs::set_permissions(extension.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let stale = extension.path().join("packs/local-assist/1.0.0");
    std::fs::create_dir_all(&stale).unwrap();
    let admitted_assist = std::fs::read_to_string("packs/cli-assist/1.0.0/assist.yaml").unwrap();
    std::fs::write(
        stale.join("assist.yaml"),
        admitted_assist.replace("id: cli-assist", "id: local-assist"),
    )
    .unwrap();
    std::fs::write(stale.join("pack.sha256"), "sha256:stale\n").unwrap();

    let shadow = extension.path().join("packs/cli-assist/1.0.0");
    std::fs::create_dir_all(&shadow).unwrap();
    std::fs::copy(
        "packs/cli-assist/1.0.0/assist.yaml",
        shadow.join("assist.yaml"),
    )
    .unwrap();
    std::fs::copy(
        "packs/cli-assist/1.0.0/pack.sha256",
        shadow.join("pack.sha256"),
    )
    .unwrap();

    let mut local_server = Server::start_dashboard_only_with_extension(extension.path());
    let response = local_server.request_without_access("GET", "/api/packs", None);
    assert_eq!(response.status, 200, "{}", response.body);
    let packs: Vec<serde_json::Value> = serde_json::from_str(&response.body).unwrap();
    let stale = packs
        .iter()
        .find(|pack| pack["id"] == "local-assist")
        .expect("missing stale local pack");
    assert_eq!(stale["source"], "local");
    assert_eq!(stale["source_label"], "ローカル（未承認・帯域未計測）");
    assert_eq!(stale["hash_matches_pin"], false);
    assert!(
        stale["warning"]
            .as_str()
            .unwrap()
            .contains("hash と pin が一致しません")
    );
    assert_eq!(stale["trial_eligible"], false);

    let shadow = packs
        .iter()
        .find(|pack| pack["id"] == "cli-assist" && pack["version"] == "1.0.0")
        .expect("missing shadowing local pack");
    assert_eq!(shadow["source"], "local");
    assert_eq!(shadow["shadowing_repository"], true);
    assert!(shadow["warning"].as_str().unwrap().contains("ローカル優先"));
    local_server.stop();
}

#[cfg(unix)]
#[test]
fn extension_supply_api_enforces_auth_origin_and_the_full_pack_lifecycle() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = tempfile::tempdir().unwrap();
    let extension = tempfile::tempdir().unwrap();
    std::fs::set_permissions(extension.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let cli = workspace.path().join("capture-pack-cli.sh");
    write_pack_capture_cli(&cli);
    let mut server =
        Server::start_with_extension_and_cli(workspace.path(), extension.path(), true, &cli);
    let assist = std::fs::read_to_string("packs/cli-assist/1.0.0/assist.yaml")
        .unwrap()
        .replace("id: cli-assist", "id: local-supply");
    let stage = serde_json::json!({
        "id": "local-supply",
        "version": "1.0.0",
        "files": {
            "assist.yaml": assist,
            "materials/CONVENTIONS.md": "# Local conventions\n"
        }
    });
    let origin = format!("http://127.0.0.1:{}", server.port);

    let response = server.request_with_access(
        "POST",
        "/api/extensions/packs",
        Some(&stage),
        None,
        Some(&origin),
    );
    assert_eq!(response.status, 401, "{}", response.body);
    assert_eq!(response.json()["code"], "trial_token_invalid");

    let response = server.request_with_access(
        "POST",
        "/api/extensions/packs",
        Some(&stage),
        Some(TEST_TRIAL_TOKEN),
        None,
    );
    assert_eq!(response.status, 403, "{}", response.body);
    assert_eq!(response.json()["code"], "trial_origin_not_allowed");

    let response = server.request("POST", "/api/extensions/packs", Some(&stage));
    assert_eq!(response.status, 200, "{}", response.body);
    let staged = response.json();
    let hash = staged["hash"].as_str().unwrap().to_string();
    assert!(hash.starts_with("sha256:"));
    assert_eq!(staged["status"], "staged");
    assert_eq!(staged["conformance"]["status"], "conformant");
    assert_eq!(staged["scrub"]["status"], "clean");

    let response = server.request_without_access("GET", "/api/extensions/packs", None);
    assert_eq!(response.status, 401, "{}", response.body);

    let response = server.request("GET", "/api/extensions/packs", None);
    assert_eq!(response.status, 200, "{}", response.body);
    let rows = response.json();
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["status"], "staged");
    assert_eq!(rows[0]["conformance_ok"], true);

    let response = server.request(
        "POST",
        "/api/extensions/packs/local-supply/1.0.0/verify",
        None,
    );
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(response.json()["hash"], hash);

    let response = server.request(
        "POST",
        "/api/extensions/packs/local-supply/1.0.0/pin",
        Some(&serde_json::json!({"hash": "sha256:not-the-observed-hash"})),
    );
    assert_eq!(response.status, 422, "{}", response.body);
    assert_eq!(response.json()["code"], "extension_verification_failed");
    assert_eq!(response.json()["report"]["hash"], hash);

    let response = server.request(
        "POST",
        "/api/extensions/packs/local-supply/1.0.0/pin",
        Some(&serde_json::json!({"hash": hash})),
    );
    assert_eq!(response.status, 204, "{}", response.body);

    let response = server.request(
        "POST",
        "/api/extensions/packs/local-supply/1.0.0/pin",
        Some(&serde_json::json!({"hash": staged["hash"]})),
    );
    assert_eq!(response.status, 409, "{}", response.body);
    assert_eq!(response.json()["code"], "extension_conflict");

    let response = server.request_without_access("GET", "/api/pack-options", None);
    assert_eq!(response.status, 200, "{}", response.body);
    let local = response.json()["packs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|pack| pack["id"] == "local-supply")
        .cloned()
        .expect("pinned local pack must be a Trial option");
    assert_eq!(local["source"], "local");
    assert_eq!(local["hash"], staged["hash"]);

    let mut proposal = session_spec();
    proposal["pack"] = serde_json::json!("local-supply@1.0.0");
    let response = server.request("POST", "/api/session-proposals", Some(&proposal));
    assert_eq!(response.status, 200, "{}", response.body);
    let proposal_response = response.json();
    assert_eq!(proposal_response["identity"]["pack"]["source"], "local");
    assert_eq!(
        proposal_response["identity"]["pack"]["hash"],
        staged["hash"]
    );

    proposal["confirmation_hash"] = proposal_response["card_hash"].clone();
    let response = server.request("POST", "/api/sessions", Some(&proposal));
    assert_eq!(response.status, 202, "{}", response.body);
    let session_id = response.json()["id"].as_str().unwrap().to_string();
    let delegated_env_path = workspace
        .path()
        .join(".anvil/runs")
        .join(&session_id)
        .join("delegated-env.txt");
    let expected_env = [
        "COMMANDAGENT_PACK_ID=local-supply".to_string(),
        "COMMANDAGENT_PACK_VERSION=1.0.0".to_string(),
        format!(
            "COMMANDAGENT_PACK_HASH={}",
            staged["hash"].as_str().unwrap()
        ),
        format!(
            "COMMANDAGENT_PACK_DIRECTORY={}",
            extension
                .path()
                .canonicalize()
                .unwrap()
                .join("packs/local-supply/1.0.0")
                .display()
        ),
    ];
    let deadline = Instant::now() + Duration::from_secs(5);
    let delegated_env = loop {
        match std::fs::read_to_string(&delegated_env_path) {
            Ok(contents)
                if expected_env
                    .iter()
                    .all(|expected| contents.contains(expected)) =>
            {
                break contents;
            }
            Ok(contents) => assert!(
                Instant::now() < deadline,
                "local pack delegation timed out: {contents:?}"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => assert!(
                Instant::now() < deadline,
                "local pack delegation timed out before the environment file appeared"
            ),
            Err(error) => panic!("could not read delegated environment: {error}"),
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    for expected in expected_env {
        assert!(delegated_env.contains(&expected), "{delegated_env}");
    }

    let response = server.request(
        "POST",
        "/api/extensions/packs/local-supply/1.0.0/retire",
        None,
    );
    assert_eq!(response.status, 204, "{}", response.body);

    let response = server.request("GET", "/api/extensions/packs", None);
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(response.json()[0]["status"], "retired");
    assert_eq!(response.json()[0]["hash"], staged["hash"]);

    let response = server.request("GET", "/api/extensions/packs/local-supply/1.0.0", None);
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(
        response.json()["files"]["materials/CONVENTIONS.md"],
        "# Local conventions\n"
    );
    assert_eq!(response.json()["report"]["status"], "retired");

    let response = server.request("POST", "/api/extensions/packs", Some(&stage));
    assert_eq!(response.status, 409, "{}", response.body);
    assert_eq!(response.json()["code"], "extension_conflict");

    let response = server.request_without_access("GET", "/api/pack-options", None);
    assert!(
        response.json()["packs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|pack| pack["id"] != "local-supply")
    );

    let journal = std::fs::read_to_string(extension.path().join("journal.jsonl")).unwrap();
    for action in ["stage", "verify", "pin", "retire"] {
        assert!(
            journal
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .any(|entry| entry["action"] == action && entry["result"] == "ok"),
            "missing successful {action} journal entry: {journal}"
        );
    }
    server.stop();
}

#[cfg(unix)]
#[test]
fn extension_supply_api_rejects_disabled_invalid_and_oversize_requests() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = tempfile::tempdir().unwrap();
    let mut disabled = Server::start(
        workspace.path(),
        std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
    );
    let response = disabled.request("GET", "/api/extensions/packs", None);
    assert_eq!(response.status, 503, "{}", response.body);
    assert_eq!(response.json()["code"], "extensions_disabled");
    disabled.stop();

    let extension = tempfile::tempdir().unwrap();
    std::fs::set_permissions(extension.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut server = Server::start_with_extension(workspace.path(), extension.path(), true);
    let assist = std::fs::read_to_string("packs/cli-assist/1.0.0/assist.yaml").unwrap();
    for request in [
        serde_json::json!({
            "id": "../escape",
            "version": "1.0.0",
            "files": {"assist.yaml": assist.clone()}
        }),
        serde_json::json!({
            "id": "local-supply",
            "version": "1.0.0",
            "files": {"../assist.yaml": assist}
        }),
    ] {
        let response = server.request("POST", "/api/extensions/packs", Some(&request));
        assert_eq!(response.status, 400, "{}", response.body);
        assert_eq!(response.json()["code"], "extension_invalid_request");
    }

    let oversize = serde_json::json!({
        "id": "local-supply",
        "version": "1.0.0",
        "files": {"assist.yaml": "x".repeat(1024 * 1024)}
    });
    let response = server.request("POST", "/api/extensions/packs", Some(&oversize));
    assert_eq!(response.status, 413, "{}", response.body);
    assert_eq!(response.json()["code"], "extension_invalid_request");
    server.stop();
}

#[cfg(unix)]
#[test]
fn run_index_reports_total_before_limit_and_normalized_status_state() {
    let repository = tempfile::tempdir().unwrap();
    let runs_root = repository.path().join("workspace/management/runs");
    for index in 0..101 {
        let run_root = runs_root.join(format!("run-{index:03}"));
        std::fs::create_dir_all(&run_root).unwrap();
        if index == 100 {
            std::fs::write(
                run_root.join("uat-report.md"),
                "# Acceptance\n\nStatus: **FULL 3/3 (2026-08-03)**\n",
            )
            .unwrap();
        }
    }
    let mut server = Server::start_dashboard_only_at_repository_root(repository.path());

    let response = server.request_without_access("GET", "/api/runs", None);
    assert_eq!(response.status, 200, "{}", response.body);
    let index: serde_json::Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(index["total"], 101);
    let runs = index["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 100);
    assert!(runs.iter().all(|run| run["id"] != "run-000"));

    let summary = runs
        .iter()
        .find(|run| run["id"] == "run-100")
        .unwrap_or_else(|| panic!("missing normalized fixture: {runs:?}"));
    let fields = summary.as_object().unwrap();
    for field in [
        "id",
        "modified_epoch_seconds",
        "report_path",
        "status",
        "status_text",
        "state",
    ] {
        assert!(fields.contains_key(field), "missing {field}: {summary}");
    }
    assert_eq!(fields.len(), 6, "unexpected RunSummary schema: {summary}");
    assert_eq!(summary["status"], "FULL 3/3 (2026-08-03)");
    assert_eq!(summary["status_text"], "FULL 3/3 (2026-08-03)");
    assert_eq!(summary["state"], "pass");
    assert_eq!(
        summary["report_path"],
        "workspace/management/runs/run-100/uat-report.md"
    );
    server.stop();
}

#[test]
fn gui_server_requires_a_runtime_token_when_trial_token_auth_is_on() {
    let workspace = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_gui_server"))
        .args(["--port", "0", "--base-path", "/"])
        .arg("--repository-root")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("--execution-root")
        .arg(workspace.path())
        .args(["--trial-token-auth", "on"])
        .env_remove("GUI_TRIAL_TOKEN")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("GUI_TRIAL_TOKEN is required when --trial-token-auth is on"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn gui_server_defaults_trial_token_auth_to_off() {
    let workspace = tempfile::tempdir().unwrap();
    let mut server = Server::start_without_trial_token(
        workspace.path(),
        std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
    );

    let runtime = server.request_without_access("GET", "/api/runtime-status", None);
    assert_eq!(runtime.status, 200, "{}", runtime.body);
    let runtime: serde_json::Value = serde_json::from_str(&runtime.body).unwrap();
    assert_eq!(runtime["trial_available"], true);
    assert_eq!(runtime["trial_token_auth_enabled"], false);
    for prerequisite in [
        "execution_root",
        "commandagent_binary",
        "trial_authentication",
    ] {
        assert_eq!(runtime["prerequisites"][prerequisite]["status"], "ready");
    }

    let index = server.request_without_access("GET", "/api/sessions", None);
    assert_eq!(index.status, 200, "{}", index.body);

    let origin = format!("http://127.0.0.1:{}", server.port);
    let proposal = server.request_with_access(
        "POST",
        "/api/session-proposals",
        Some(&session_spec()),
        None,
        Some(&origin),
    );
    assert_eq!(proposal.status, 200, "{}", proposal.body);

    let cross_site_unsafe =
        server.request_without_access("POST", "/api/session-proposals", Some(&session_spec()));
    assert_eq!(cross_site_unsafe.status, 403, "{}", cross_site_unsafe.body);
    assert_error(
        &cross_site_unsafe,
        "trial_origin_not_allowed",
        "trial request origin is not allowed",
    );
    server.stop();
}

#[cfg(unix)]
#[test]
fn gui_server_rejects_repository_workspace_overlap_and_symlink_aliases() {
    use std::os::unix::fs::symlink;

    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let alias = temp.path().join("repository-alias");
    let repository_child = repository.join("gui");
    symlink(repository, &alias).unwrap();
    for workspace in [
        repository,
        repository.parent().unwrap(),
        repository_child.as_path(),
        &alias,
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_gui_server"))
            .args(["--port", "0", "--base-path", "/"])
            .arg("--repository-root")
            .arg(repository)
            .arg("--execution-root")
            .arg(workspace)
            .env("GUI_TRIAL_TOKEN", TEST_TRIAL_TOKEN)
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {}", workspace.display());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("must be disjoint"),
            "stderr for {}: {}",
            workspace.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(unix)]
#[test]
fn gui_server_revalidates_the_workspace_before_dispatch() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    let alias = temp.path().join("trial-workspace");
    symlink(&first, &alias).unwrap();
    let mut server = Server::start(
        &alias,
        std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
    );
    let spec = session_spec();
    let proposal = server.request("POST", "/api/session-proposals", Some(&spec));
    assert_eq!(proposal.status, 200, "{}", proposal.body);
    let proposal: serde_json::Value = serde_json::from_str(&proposal.body).unwrap();

    std::fs::remove_file(&alias).unwrap();
    symlink(&second, &alias).unwrap();
    let mut confirmed = spec;
    confirmed["confirmation_hash"] = proposal["card_hash"].clone();
    let response = server.request("POST", "/api/sessions", Some(&confirmed));
    assert_eq!(response.status, 409, "{}", response.body);
    assert_eq!(response.json()["code"], "trial_workspace_conflict");
    assert!(response.body.contains("changed after startup"));
    server.stop();
}

#[cfg(unix)]
#[test]
fn spawn_failure_reports_the_binary_and_releases_the_workspace() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let cli = temp.path().join("missing-commandagent");
    let mut server = Server::start(&workspace, &cli);
    let spec = session_spec();
    let proposal = server.request("POST", "/api/session-proposals", Some(&spec));
    assert_eq!(proposal.status, 200, "{}", proposal.body);
    let proposal: serde_json::Value = serde_json::from_str(&proposal.body).unwrap();
    let mut confirmed = spec;
    confirmed["confirmation_hash"] = proposal["card_hash"].clone();

    let failed = server.request("POST", "/api/sessions", Some(&confirmed));
    assert_eq!(failed.status, 500, "{}", failed.body);
    assert_eq!(failed.json()["code"], "trial_internal_error");
    assert!(
        failed.body.contains(cli.to_string_lossy().as_ref()),
        "{}",
        failed.body
    );
    assert!(
        failed.body.contains("No such file or directory") && failed.body.contains("os error"),
        "{}",
        failed.body
    );
    let lease = server.request("GET", "/api/trial-workspace", None);
    assert_eq!(lease.status, 200, "{}", lease.body);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&lease.body).unwrap(),
        serde_json::json!({ "status": "idle" })
    );
    server.stop();

    write_terminal_cli(&cli);
    let mut restarted = Server::start(&workspace, &cli);
    let lease = restarted.request("GET", "/api/trial-workspace", None);
    assert_eq!(lease.status, 200, "{}", lease.body);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&lease.body).unwrap(),
        serde_json::json!({ "status": "idle" })
    );
    let created = restarted.request("POST", "/api/sessions", Some(&confirmed));
    assert_eq!(created.status, 202, "{}", created.body);
    restarted.stop();
}

#[cfg(unix)]
#[test]
fn recovery_required_lease_is_exposed_by_an_authenticated_get() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let session_id = "0198b9c8-fab8-7000-8000-000000000064";
    std::fs::create_dir_all(
        workspace
            .join(".anvil/runs")
            .join(session_id)
            .join("state/boundary-confirmations"),
    )
    .unwrap();
    let mut server = Server::start(
        &workspace,
        std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
    );

    let unauthorized = server.request_without_access("GET", "/api/trial-workspace", None);
    assert_eq!(unauthorized.status, 401, "{}", unauthorized.body);
    let lease = server.request("GET", "/api/trial-workspace", None);
    assert_eq!(lease.status, 200, "{}", lease.body);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&lease.body).unwrap(),
        serde_json::json!({
            "status": "recovery_required",
            "session_id": session_id,
        })
    );

    let spec = session_spec();
    let proposal = server.request("POST", "/api/session-proposals", Some(&spec));
    assert_eq!(proposal.status, 200, "{}", proposal.body);
    let proposal = proposal.json();
    let mut confirmed = spec;
    confirmed["confirmation_hash"] = proposal["card_hash"].clone();
    let blocked = server.request("POST", "/api/sessions", Some(&confirmed));
    assert_eq!(blocked.status, 409, "{}", blocked.body);
    assert_error(
        &blocked,
        "trial_workspace_recovery_required",
        &format!("trial workspace requires recovery for non-terminal session {session_id}"),
    );
    server.stop();
}

#[cfg(unix)]
#[test]
fn session_index_requires_authentication_tracks_directories_and_caps_results() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let mut server = Server::start(
        &workspace,
        std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
    );

    let unauthorized = server.request_without_access("GET", "/api/sessions", None);
    assert_eq!(unauthorized.status, 401, "{}", unauthorized.body);
    let empty = server.request("GET", "/api/sessions", None);
    assert_eq!(empty.status, 200, "{}", empty.body);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&empty.body).unwrap(),
        serde_json::json!({ "sessions": [], "lease": { "status": "idle" } })
    );

    let runs_root = workspace.join(".anvil/runs");
    let outside_events = workspace.join("outside-events.jsonl");
    std::fs::write(
        &outside_events,
        "{\"event\":\"tui_command_stop\",\"status\":\"completed\"}\n",
    )
    .unwrap();
    let mut ids = Vec::new();
    for value in 1..=102u128 {
        let id = uuid::Uuid::from_u128(value).to_string();
        let run_root = runs_root.join(&id);
        let confirmations = run_root.join("state/boundary-confirmations");
        std::fs::create_dir_all(&confirmations).unwrap();
        std::fs::write(confirmations.join("fixture.json"), "{}\n").unwrap();
        match value {
            97 => symlink(&outside_events, run_root.join("events.jsonl")).unwrap(),
            98 => std::fs::write(
                run_root.join("events.jsonl"),
                "{\"event\":\"tui_command_stop\",\"status\":\"failed\",\"ok\":false}\n{\"event\":\"run_stop\",\"status\":\"completed\",\"ok\":true}\n",
            )
            .unwrap(),
            99 => std::fs::write(run_root.join("events.jsonl"), "not-json\n").unwrap(),
            100 => std::fs::write(
                run_root.join("events.jsonl"),
                "{\"event\":\"tui_command_stop\",\"status\":\"completed\",\"ok\":true,\"assurance_level\":\"full\",\"final_acceptance_status\":\"full_success\"}\n",
            )
            .unwrap(),
            101 => std::fs::write(
                run_root.join("events.jsonl"),
                "{\"event\":\"ultra_phase_started\"}\n",
            )
            .unwrap(),
            _ => {}
        }
        ids.push(id);
    }
    let unrelated = runs_root.join("not-a-session/state/boundary-confirmations");
    std::fs::create_dir_all(&unrelated).unwrap();
    std::fs::write(unrelated.join("fixture.json"), "{}\n").unwrap();

    let populated = server.request("GET", "/api/sessions", None);
    assert_eq!(populated.status, 200, "{}", populated.body);
    let populated: serde_json::Value = serde_json::from_str(&populated.body).unwrap();
    let sessions = populated["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 100);
    assert_eq!(populated["lease"], serde_json::json!({ "status": "idle" }));
    for (id, gate, status) in [
        (&ids[96], serde_json::Value::Null, "unreadable"),
        (&ids[97], serde_json::json!("gate_4"), "failed"),
        (&ids[98], serde_json::Value::Null, "unreadable"),
        (&ids[99], serde_json::json!("gate_3"), "completed"),
        (&ids[100], serde_json::json!("gate_2"), "running"),
        (&ids[101], serde_json::json!("gate_2"), "starting"),
    ] {
        let summary = sessions
            .iter()
            .find(|session| session["id"] == id.as_str())
            .unwrap_or_else(|| panic!("missing session {id}: {sessions:?}"));
        assert_eq!(summary["gate"], gate);
        assert_eq!(summary["status"], status);
        assert!(summary["started_epoch_seconds"].as_u64().unwrap() > 0);
        assert!(summary["modified_epoch_seconds"].as_u64().unwrap() > 0);
    }

    std::fs::remove_dir_all(runs_root.join(&ids[101])).unwrap();
    let after_removal = server.request("GET", "/api/sessions", None);
    assert_eq!(after_removal.status, 200, "{}", after_removal.body);
    let after_removal: serde_json::Value = serde_json::from_str(&after_removal.body).unwrap();
    assert!(
        after_removal["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|session| session["id"] != ids[101])
    );
    server.stop();
}

#[cfg(unix)]
#[test]
fn confirmed_session_delegates_with_cli_event_bytes_unchanged() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(workspace.join("cli")).unwrap();
    std::fs::write(workspace.join("cli/main.py"), "print('fixture')\n").unwrap();
    let cli = temp.path().join("fake-commandagent");
    let fixture = include_str!("fixtures/gui_cli_events.jsonl");
    let script = format!(
        "#!/bin/sh\nenv | sort > \"${{COMMANDAGENT_EVAL_EVENTS%/*}}/delegated-env.txt\"\ncase \" $* \" in\n  *\" --run-ultra-plan \"*) sleep 1; printf '%s' '{}' >> \"$COMMANDAGENT_EVAL_EVENTS\" ;;\n  *\" --ultra-plan-run \"*) sleep 1; printf '%s' '{}' > \"$COMMANDAGENT_EVAL_EVENTS\" ;;\n  *) printf '%s' '{}' > \"$COMMANDAGENT_EVAL_EVENTS\" ;;\nesac\n",
        fixture.replace('\'', "'\\''"),
        fixture.replace('\'', "'\\''"),
        fixture.replace('\'', "'\\''")
    );
    std::fs::write(&cli, script).unwrap();
    let mut permissions = std::fs::metadata(&cli).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&cli, permissions).unwrap();

    let direct_events = temp.path().join("direct-events.jsonl");
    assert!(
        Command::new(&cli)
            .env("COMMANDAGENT_EVAL_EVENTS", &direct_events)
            .status()
            .unwrap()
            .success()
    );
    let direct_bytes = std::fs::read(&direct_events).unwrap();
    let mut server = Server::start_with_delegate_env(
        &workspace,
        &cli,
        &[
            ("OPENAI_API_KEY", "allowlisted-provider-key"),
            ("COMMANDAGENT_PACK_DIRECTORY", "/tmp/untrusted-pack"),
            ("COMMANDAGENT_PACK_ID", "untrusted-pack"),
            ("COMMANDAGENT_PACK_VERSION", "9.9.9"),
            ("COMMANDAGENT_PACK_HASH", "sha256:untrusted"),
            (
                "COMMANDAGENT_TEST_UNRELATED_PARENT_SECRET",
                "must-not-cross",
            ),
        ],
    );
    let idle = server.request_without_access("GET", "/api/runtime-status", None);
    assert_eq!(idle.status, 200, "{}", idle.body);
    let idle: serde_json::Value = serde_json::from_str(&idle.body).unwrap();
    assert_eq!(idle["trial_available"], true);
    assert_eq!(idle["trial_token_auth_enabled"], true);
    assert!(idle["session"].is_null());
    assert_eq!(
        idle["prerequisites"]["trial_authentication"]["status"],
        "action_required"
    );
    let spec = serde_json::json!({
        "goal": "Create a CLI --pattern filter command",
        "profile": "python-cli",
        "provider": "ollama",
        "model": "fixture-executor",
        "planner_provider": "ollama",
        "planner_model": "fixture-planner",
        "pack": "cli-assist@1.0.0"
    });

    let proposal = server.request("POST", "/api/session-proposals", Some(&spec));
    assert_eq!(proposal.status, 200, "{}", proposal.body);
    let proposal_json: serde_json::Value = serde_json::from_str(&proposal.body).unwrap();
    let card_hash = proposal_json["card_hash"].as_str().unwrap();
    assert_eq!(proposal_json["confirmation_required"], true);
    assert_eq!(
        proposal_json["identity"]["contract_checks"],
        serde_json::json!(["C1", "C2", "C3", "C4"])
    );
    assert_eq!(
        proposal_json["identity"]["pack"],
        serde_json::json!({
            "selection": "pinned",
            "id": "cli-assist",
            "version": "1.0.0",
            "hash": ADMITTED_PACKS[0].hash,
            "point": "cli-validation",
            "source": "admitted"
        })
    );
    let card_markdown = proposal_json["card_markdown"].as_str().unwrap();
    for expected in [
        "# Gate 1 — 実行前の確認",
        "C1 — 実行動作",
        "C2 — ヘルプの正確さ",
        "C3 — 出力の正確さ",
        "C4 — 再現性",
        "追加の検証パック: cli-assist@1.0.0",
        "検証パックの供給元: 承認済み",
        card_hash,
    ] {
        assert!(card_markdown.contains(expected), "{card_markdown}");
    }
    assert_eq!(proposal_json["price"]["duration_n"], 3);
    assert_eq!(proposal_json["price"]["cost_n"], 0);
    assert_eq!(
        proposal_json["identity"]["workspace"],
        workspace.canonicalize().unwrap().to_string_lossy().as_ref()
    );

    let unauthorized = server.request_without_access("POST", "/api/session-proposals", Some(&spec));
    assert_eq!(unauthorized.status, 401, "{}", unauthorized.body);
    assert_error(
        &unauthorized,
        "trial_token_invalid",
        "a valid GUI trial bearer token is required",
    );
    let forbidden_origin = server.request_with_access(
        "POST",
        "/api/session-proposals",
        Some(&spec),
        Some(TEST_TRIAL_TOKEN),
        Some("https://attacker.invalid"),
    );
    assert_eq!(forbidden_origin.status, 403, "{}", forbidden_origin.body);
    assert_error(
        &forbidden_origin,
        "trial_origin_not_allowed",
        "trial request origin is not allowed",
    );

    let denied = server.request("POST", "/api/sessions", Some(&spec));
    assert_eq!(denied.status, 428, "{}", denied.body);
    assert_error(
        &denied,
        "trial_confirmation_required",
        "Gate 1 confirmation_hash is required before dispatch",
    );

    let mut changed_pack = spec.clone();
    changed_pack["pack"] = serde_json::json!("cli-assist@1.1.0");
    let changed_proposal = server.request("POST", "/api/session-proposals", Some(&changed_pack));
    assert_eq!(changed_proposal.status, 200, "{}", changed_proposal.body);
    let changed_hash = changed_proposal.json()["card_hash"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(changed_hash, card_hash);
    let mut stale_confirmation = changed_pack;
    stale_confirmation["confirmation_hash"] = serde_json::json!(card_hash);
    let stale = server.request("POST", "/api/sessions", Some(&stale_confirmation));
    assert_eq!(stale.status, 412, "{}", stale.body);
    assert_error(
        &stale,
        "trial_confirmation_stale",
        "Gate 1 card changed; request and confirm the current card",
    );

    let mut confirmed = spec.clone();
    confirmed["confirmation_hash"] = serde_json::Value::String(card_hash.to_string());
    let created = server.request("POST", "/api/sessions", Some(&confirmed));
    assert_eq!(created.status, 202, "{}", created.body);
    let created_json: serde_json::Value = serde_json::from_str(&created.body).unwrap();
    let id = created_json["id"].as_str().unwrap();
    let started_epoch_seconds = created_json["started_epoch_seconds"].as_u64().unwrap();
    assert!(started_epoch_seconds > 0);
    let delegated_events = workspace.join(".anvil/runs").join(id).join("events.jsonl");

    let running = server.request_without_access("GET", "/api/runtime-status", None);
    assert_eq!(running.status, 200, "{}", running.body);
    let running: serde_json::Value = serde_json::from_str(&running.body).unwrap();
    assert_eq!(running["trial_available"], true);
    assert_eq!(running["session"]["id"], id);
    assert_eq!(running["session"]["state"], "running");

    let competing = server.request("POST", "/api/sessions", Some(&confirmed));
    assert_eq!(competing.status, 409, "{}", competing.body);
    assert_error(
        &competing,
        "trial_workspace_running",
        &format!("trial workspace is already running session {id}"),
    );

    let index = server.request("GET", "/api/sessions", None);
    assert_eq!(index.status, 200, "{}", index.body);
    let index: serde_json::Value = serde_json::from_str(&index.body).unwrap();
    assert_eq!(index["lease"]["status"], "running");
    assert_eq!(index["lease"]["session_id"], id);
    let sessions = index["sessions"].as_array().unwrap();
    assert_eq!(sessions.first().unwrap()["id"], id);
    assert_eq!(sessions.first().unwrap()["gate"], "gate_2");
    assert_eq!(
        sessions.first().unwrap()["pack"],
        serde_json::json!({
            "id": "cli-assist",
            "version": "1.0.0",
            "hash": ADMITTED_PACKS[0].hash,
            "source": "admitted",
            "source_label": "承認済み"
        })
    );
    assert!(
        sessions.first().unwrap()["started_epoch_seconds"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        sessions.first().unwrap()["modified_epoch_seconds"]
            .as_u64()
            .unwrap()
            > 0
    );

    let deadline = Instant::now() + Duration::from_secs(5);
    while std::fs::read(&delegated_events).ok().as_deref() != Some(direct_bytes.as_slice())
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(std::fs::read(&delegated_events).unwrap(), direct_bytes);
    let delegated_env =
        std::fs::read_to_string(delegated_events.parent().unwrap().join("delegated-env.txt"))
            .unwrap();
    assert!(delegated_env.contains("PATH="), "{delegated_env}");
    assert!(
        delegated_env.contains("OPENAI_API_KEY=allowlisted-provider-key"),
        "{delegated_env}"
    );
    for expected in [
        "COMMANDAGENT_PACK_ID=cli-assist".to_string(),
        "COMMANDAGENT_PACK_VERSION=1.0.0".to_string(),
        format!("COMMANDAGENT_PACK_HASH={}", ADMITTED_PACKS[0].hash),
        "COMMANDAGENT_PACK_DIRECTORY=".to_string(),
    ] {
        assert!(delegated_env.contains(&expected), "{delegated_env}");
    }
    for forbidden in [
        "COMMANDAGENT_PACK_ID=untrusted-pack",
        "COMMANDAGENT_PACK_VERSION=9.9.9",
        "COMMANDAGENT_PACK_HASH=sha256:untrusted",
        "COMMANDAGENT_PACK_DIRECTORY=/tmp/untrusted-pack",
        "COMMANDAGENT_TEST_UNRELATED_PARENT_SECRET",
        "GUI_TRIAL_TOKEN=",
    ] {
        assert!(
            !delegated_env.contains(forbidden),
            "delegated environment contains {forbidden}: {delegated_env}"
        );
    }

    let status = server.request("GET", &format!("/api/sessions/{id}"), None);
    assert_eq!(status.status, 200, "{}", status.body);
    assert_eq!(status.header("cache-control"), Some("private, no-cache"));
    let etag = status.header("etag").unwrap().to_string();
    assert!(etag.starts_with("W/\""), "{etag}");
    let status_json: serde_json::Value = serde_json::from_str(&status.body).unwrap();
    let status_keys = status_json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        status_keys,
        std::collections::BTreeSet::from([
            "acceptance_sheet",
            "assurance",
            "average_duration_seconds",
            "event_count",
            "events_path",
            "gate",
            "id",
            "identity",
            "phases",
            "section5",
            "started_epoch_seconds",
            "status",
            "verdict",
        ])
    );
    assert_eq!(status_json["started_epoch_seconds"], started_epoch_seconds);
    assert_eq!(
        status_json["average_duration_seconds"],
        proposal_json["price"]["average_duration_seconds"]
    );
    assert_eq!(status_json["gate"], "gate_3");
    assert_eq!(status_json["verdict"], "full");
    assert_eq!(status_json["identity"]["request"], spec["goal"]);
    assert_eq!(status_json["identity"]["profile"], spec["profile"]);
    assert_eq!(
        status_json["identity"]["pins"],
        serde_json::json!({
            "planner_provider": "ollama",
            "planner_model": "fixture-planner",
            "executor_provider": "ollama",
            "executor_model": "fixture-executor",
            "preset": "profile"
        })
    );
    assert_eq!(
        status_json["identity"]["pack"],
        proposal_json["identity"]["pack"]
    );
    assert_eq!(status_json["phases"][0]["status"], "completed");
    assert!(
        status_json["acceptance_sheet"]
            .as_str()
            .unwrap()
            .contains("# D-3c acceptance sheet")
    );
    let acceptance_sheet = status_json["acceptance_sheet"].as_str().unwrap();
    assert!(acceptance_sheet.contains("Pack: cli-assist@1.0.0"));
    assert!(acceptance_sheet.contains("Pack source: 承認済み"));
    let unchanged = server.request_if_none_match(&format!("/api/sessions/{id}"), &etag);
    assert_eq!(unchanged.status, 304, "{}", unchanged.body);
    assert!(unchanged.body.is_empty(), "304 response had a body");
    assert_eq!(unchanged.header("etag"), Some(etag.as_str()));
    let completed = server.request_without_access("GET", "/api/runtime-status", None);
    assert_eq!(completed.status, 200, "{}", completed.body);
    let completed: serde_json::Value = serde_json::from_str(&completed.body).unwrap();
    assert_eq!(completed["trial_available"], true);
    assert!(completed["session"].is_null());

    let credential = serde_json::json!({
        "directive": format!("use token={}", "a".repeat(24))
    });
    let rejected = server.request(
        "POST",
        &format!("/api/sessions/{id}/directives"),
        Some(&credential),
    );
    assert_eq!(rejected.status, 422, "{}", rejected.body);

    let directive_request = serde_json::json!({ "directive": "Keep the output sorted" });
    let proposed = server.request(
        "POST",
        &format!("/api/sessions/{id}/directives"),
        Some(&directive_request),
    );
    assert_eq!(proposed.status, 200, "{}", proposed.body);
    let proposed_json: serde_json::Value = serde_json::from_str(&proposed.body).unwrap();
    assert_eq!(
        proposed_json["scrubbed_directive"],
        "Keep the output sorted"
    );
    assert!(
        proposed_json["directive_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    let duplicate = server.request(
        "POST",
        &format!("/api/sessions/{id}/directives"),
        Some(&directive_request),
    );
    assert_eq!(duplicate.status, 409, "{}", duplicate.body);
    let wrong_hash = format!("sha256:{}", "f".repeat(64));
    let unconfirmed = server.request(
        "POST",
        &format!("/api/sessions/{id}/directives/{wrong_hash}"),
        None,
    );
    assert_eq!(unconfirmed.status, 400, "{}", unconfirmed.body);

    let directive_hash = proposed_json["directive_hash"].as_str().unwrap();
    let continued = server.request(
        "POST",
        &format!("/api/sessions/{id}/directives/{directive_hash}"),
        None,
    );
    assert_eq!(continued.status, 202, "{}", continued.body);
    let intervention = server.request(
        "POST",
        &format!("/api/sessions/{id}/directives"),
        Some(&directive_request),
    );
    assert_eq!(intervention.status, 409, "{}", intervention.body);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let completed = server.request("GET", &format!("/api/sessions/{id}"), None);
        assert_eq!(completed.status, 200, "{}", completed.body);
        let completed_json: serde_json::Value = serde_json::from_str(&completed.body).unwrap();
        if completed_json["gate"] != "gate_2" {
            assert_eq!(completed_json["gate"], "gate_3");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "directive continuation timed out"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    server.stop();
}

#[cfg(unix)]
#[test]
fn trial_session_files_are_authenticated_confined_and_bounded() {
    use std::os::unix::fs::symlink;

    const SESSION_ID: &str = "018f0e32-7b80-7000-8000-000000000070";
    const ALIAS_SESSION_ID: &str = "018f0e32-7b80-7000-8000-000000000071";

    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let run_root = workspace.join(".anvil/runs").join(SESSION_ID);
    std::fs::create_dir_all(run_root.join("evidence")).unwrap();
    std::fs::write(
        run_root.join("summary.md"),
        "# Trial summary\nfailed honestly\n",
    )
    .unwrap();
    std::fs::write(
        run_root.join("evidence/acceptance.txt"),
        "acceptance details\n",
    )
    .unwrap();
    std::fs::write(run_root.join("oversized.txt"), vec![b'x'; 1_048_577]).unwrap();
    std::fs::write(workspace.join("outside.txt"), "outside\n").unwrap();
    symlink(
        run_root.join("summary.md"),
        run_root.join("summary-link.md"),
    )
    .unwrap();
    symlink(
        &run_root,
        workspace.join(".anvil/runs").join(ALIAS_SESSION_ID),
    )
    .unwrap();
    for index in 0..260 {
        std::fs::write(
            run_root.join(format!("evidence/artifact-{index:03}.txt")),
            format!("artifact {index}\n"),
        )
        .unwrap();
    }

    let events_path = run_root.join("events.jsonl");
    let mut events = std::fs::File::create(&events_path).unwrap();
    let padding = "x".repeat(4_040);
    for index in 0..1_040 {
        writeln!(
            events,
            "{{\"event\":\"progress\",\"index\":{index},\"padding\":\"{padding}\"}}"
        )
        .unwrap();
    }
    writeln!(events, "{{\"event\":\"penultimate\"}}").unwrap();
    writeln!(events, "{{\"event\":\"terminal\"}}").unwrap();
    drop(events);
    assert!(std::fs::metadata(&events_path).unwrap().len() > 4 * 1024 * 1024);

    let mut server = Server::start(
        &workspace,
        std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
    );
    let artifacts_path = format!("/api/sessions/{SESSION_ID}/artifacts");
    let events_endpoint = format!("/api/sessions/{SESSION_ID}/events?tail=2");

    let unauthorized = server.request_without_access("GET", &artifacts_path, None);
    assert_eq!(unauthorized.status, 401, "{}", unauthorized.body);
    let unauthorized_events = server.request_without_access("GET", &events_endpoint, None);
    assert_eq!(
        unauthorized_events.status, 401,
        "{}",
        unauthorized_events.body
    );

    let listing =
        server.request_with_access("GET", &artifacts_path, None, Some(TEST_TRIAL_TOKEN), None);
    assert_eq!(listing.status, 200, "{}", listing.body);
    let listing: serde_json::Value = serde_json::from_str(&listing.body).unwrap();
    assert_eq!(listing.as_array().unwrap().len(), 256);
    assert!(listing.as_array().unwrap().iter().all(|entry| {
        entry["path"].as_str() != Some("summary-link.md")
            && entry["path"]
                .as_str()
                .is_some_and(|path| !path.starts_with('/'))
    }));

    let summary = server.request("GET", &format!("{artifacts_path}?path=summary.md"), None);
    assert_eq!(summary.status, 200, "{}", summary.body);
    let summary: serde_json::Value = serde_json::from_str(&summary.body).unwrap();
    assert_eq!(summary["path"], "summary.md");
    assert_eq!(summary["content"], "# Trial summary\nfailed honestly\n");

    let traversal = server.request(
        "GET",
        &format!("{artifacts_path}?path=%2E%2E%2Foutside.txt"),
        None,
    );
    assert_eq!(traversal.status, 404, "{}", traversal.body);
    let symlink_read = server.request(
        "GET",
        &format!("{artifacts_path}?path=summary-link.md"),
        None,
    );
    assert_eq!(symlink_read.status, 404, "{}", symlink_read.body);
    let oversized = server.request("GET", &format!("{artifacts_path}?path=oversized.txt"), None);
    assert_eq!(oversized.status, 413, "{}", oversized.body);
    assert_eq!(oversized.json()["code"], "resource_too_large");

    let tail =
        server.request_with_access("GET", &events_endpoint, None, Some(TEST_TRIAL_TOKEN), None);
    assert_eq!(tail.status, 200, "{}", tail.body);
    let tail: serde_json::Value = serde_json::from_str(&tail.body).unwrap();
    assert_eq!(tail["path"], "events.jsonl");
    assert_eq!(
        tail["content"],
        "{\"event\":\"penultimate\"}\n{\"event\":\"terminal\"}\n"
    );
    let excessive_tail = server.request(
        "GET",
        &format!("/api/sessions/{SESSION_ID}/events?tail=2001"),
        None,
    );
    assert_eq!(excessive_tail.status, 422, "{}", excessive_tail.body);
    assert_eq!(excessive_tail.json()["code"], "trial_request_invalid");
    let invalid_id = server.request("GET", "/api/sessions/not-a-uuid/events?tail=2", None);
    assert_eq!(invalid_id.status, 404, "{}", invalid_id.body);
    let aliased_run = server.request(
        "GET",
        &format!("/api/sessions/{ALIAS_SESSION_ID}/artifacts"),
        None,
    );
    assert_eq!(aliased_run.status, 404, "{}", aliased_run.body);

    let management_runs = server.request_without_access("GET", "/api/runs", None);
    assert_eq!(management_runs.status, 200, "{}", management_runs.body);
    server.stop();
}

#[cfg(unix)]
#[test]
fn trial_session_files_reject_a_symlinked_runtime_root() {
    use std::os::unix::fs::symlink;

    const SESSION_ID: &str = "018f0e32-7b80-7000-8000-000000000072";

    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let external_anvil = temp.path().join("external-anvil");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(external_anvil.join("runs").join(SESSION_ID)).unwrap();
    std::fs::write(
        external_anvil
            .join("runs")
            .join(SESSION_ID)
            .join("summary.md"),
        "must remain outside the readable boundary\n",
    )
    .unwrap();
    symlink(&external_anvil, workspace.join(".anvil")).unwrap();

    let mut server = Server::start(
        &workspace,
        std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
    );
    let response = server.request(
        "GET",
        &format!("/api/sessions/{SESSION_ID}/artifacts?path=summary.md"),
        None,
    );
    assert_eq!(response.status, 404, "{}", response.body);
    server.stop();
}

#[cfg(unix)]
struct Server {
    child: Child,
    port: u16,
    _stdout: BufReader<std::process::ChildStdout>,
}

#[cfg(unix)]
#[derive(Clone, Copy)]
struct StaticExport<'a> {
    root: &'a std::path::Path,
    base_path: &'a str,
}

#[cfg(unix)]
impl<'a> StaticExport<'a> {
    fn new(root: &'a std::path::Path, base_path: &'a str) -> Self {
        Self { root, base_path }
    }
}

#[cfg(unix)]
impl Server {
    fn start(workspace: &std::path::Path, cli: &std::path::Path) -> Self {
        let static_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("gui/out");
        Self::start_with_workspace(Some(workspace), cli, true, &static_root, "/")
    }

    fn start_without_trial_token(workspace: &std::path::Path, cli: &std::path::Path) -> Self {
        let static_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("gui/out");
        Self::start_with_workspace(Some(workspace), cli, false, &static_root, "/")
    }

    fn start_with_delegate_env(
        workspace: &std::path::Path,
        cli: &std::path::Path,
        environment: &[(&str, &str)],
    ) -> Self {
        let static_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("gui/out");
        Self::start_with_repository_root_and_env(
            Some(workspace),
            cli,
            true,
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
            StaticExport::new(&static_root, "/"),
            None,
            environment,
        )
    }

    fn start_with_extension(
        workspace: &std::path::Path,
        extension_root: &std::path::Path,
        authenticated: bool,
    ) -> Self {
        Self::start_with_extension_and_cli(
            workspace,
            extension_root,
            authenticated,
            std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
        )
    }

    fn start_with_extension_and_cli(
        workspace: &std::path::Path,
        extension_root: &std::path::Path,
        authenticated: bool,
        cli: &std::path::Path,
    ) -> Self {
        let static_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("gui/out");
        Self::start_with_repository_root_and_env(
            Some(workspace),
            cli,
            authenticated,
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
            StaticExport::new(&static_root, "/"),
            Some(extension_root),
            &[],
        )
    }

    fn start_dashboard_only() -> Self {
        let static_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("gui/out");
        Self::start_dashboard_only_with_static_root(&static_root)
    }

    fn start_dashboard_only_with_static_root(static_root: &std::path::Path) -> Self {
        Self::start_dashboard_only_with_static_root_at_base_path(static_root, "/")
    }

    fn start_dashboard_only_with_static_root_at_base_path(
        static_root: &std::path::Path,
        base_path: &str,
    ) -> Self {
        Self::start_with_workspace(
            None,
            std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
            false,
            static_root,
            base_path,
        )
    }

    fn start_dashboard_only_at_repository_root(repository_root: &std::path::Path) -> Self {
        let static_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("gui/out");
        Self::start_with_repository_root(
            None,
            std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
            false,
            repository_root,
            &static_root,
            "/",
        )
    }

    fn start_dashboard_only_with_extension(extension_root: &std::path::Path) -> Self {
        let static_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("gui/out");
        Self::start_with_repository_root_and_env(
            None,
            std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
            false,
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
            StaticExport::new(&static_root, "/"),
            Some(extension_root),
            &[],
        )
    }

    fn start_with_workspace(
        workspace: Option<&std::path::Path>,
        cli: &std::path::Path,
        authenticated: bool,
        static_root: &std::path::Path,
        base_path: &str,
    ) -> Self {
        Self::start_with_repository_root(
            workspace,
            cli,
            authenticated,
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
            static_root,
            base_path,
        )
    }

    fn start_with_repository_root(
        workspace: Option<&std::path::Path>,
        cli: &std::path::Path,
        authenticated: bool,
        repository_root: &std::path::Path,
        static_root: &std::path::Path,
        base_path: &str,
    ) -> Self {
        Self::start_with_repository_root_and_env(
            workspace,
            cli,
            authenticated,
            repository_root,
            StaticExport::new(static_root, base_path),
            None,
            &[],
        )
    }

    fn start_with_repository_root_and_env(
        workspace: Option<&std::path::Path>,
        cli: &std::path::Path,
        authenticated: bool,
        repository_root: &std::path::Path,
        static_export: StaticExport<'_>,
        extension_root: Option<&std::path::Path>,
        environment: &[(&str, &str)],
    ) -> Self {
        let StaticExport { root, base_path } = static_export;
        let mut command = Command::new(env!("CARGO_BIN_EXE_gui_server"));
        command
            .args(["--port", "0", "--base-path", base_path])
            .arg("--repository-root")
            .arg(repository_root)
            .arg("--static-dir")
            .arg(root)
            .arg("--commandagent-bin")
            .arg(cli)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.envs(environment.iter().copied());
        if let Some(workspace) = workspace {
            command.arg("--execution-root").arg(workspace);
        }
        if let Some(extension_root) = extension_root {
            command.arg("--extension-root").arg(extension_root);
        }
        if authenticated {
            command
                .args(["--trial-token-auth", "on"])
                .env("GUI_TRIAL_TOKEN", TEST_TRIAL_TOKEN);
        } else {
            command.env_remove("GUI_TRIAL_TOKEN");
        }
        let mut child = command.spawn().unwrap();
        let mut line = String::new();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        stdout.read_line(&mut line).unwrap();
        let port = line
            .split("127.0.0.1:")
            .nth(1)
            .and_then(|tail| tail.split('/').next())
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| {
                let mut error = String::new();
                child
                    .stderr
                    .take()
                    .unwrap()
                    .read_to_string(&mut error)
                    .unwrap();
                panic!("unable to parse server address: {line}; stderr: {error}")
            });
        Self {
            child,
            port,
            _stdout: stdout,
        }
    }

    fn request(&self, method: &str, path: &str, body: Option<&serde_json::Value>) -> HttpResponse {
        let origin = format!("http://127.0.0.1:{}", self.port);
        self.request_with_access_and_headers(
            method,
            path,
            body,
            Some(TEST_TRIAL_TOKEN),
            Some(&origin),
            &[],
        )
    }

    fn request_if_none_match(&self, path: &str, etag: &str) -> HttpResponse {
        let origin = format!("http://127.0.0.1:{}", self.port);
        self.request_with_access_and_headers(
            "GET",
            path,
            None,
            Some(TEST_TRIAL_TOKEN),
            Some(&origin),
            &[("If-None-Match", etag)],
        )
    }

    fn request_without_access(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> HttpResponse {
        self.request_with_access(method, path, body, None, None)
    }

    fn request_with_access(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
        token: Option<&str>,
        origin: Option<&str>,
    ) -> HttpResponse {
        self.request_with_access_and_headers(method, path, body, token, origin, &[])
    }

    fn request_with_access_and_headers(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
        token: Option<&str>,
        origin: Option<&str>,
        extra_headers: &[(&str, &str)],
    ) -> HttpResponse {
        let body = body.map(ToString::to_string).unwrap_or_default();
        let authorization = token
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        let origin = origin
            .map(|origin| format!("Origin: {origin}\r\n"))
            .unwrap_or_default();
        let extra_headers = extra_headers
            .iter()
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect::<String>();
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n{authorization}{origin}{extra_headers}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            self.port,
            body.len()
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        let status = head
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse::<u16>()
            .unwrap();
        let headers = head
            .lines()
            .skip(1)
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_string()))
            .collect();
        HttpResponse {
            status,
            headers,
            body: body.to_string(),
        }
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(unix)]
impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(unix)]
struct HttpResponse {
    status: u16,
    headers: BTreeMap<String, String>,
    body: String,
}

#[cfg(unix)]
impl HttpResponse {
    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap()
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

#[cfg(unix)]
fn assert_error(response: &HttpResponse, code: &str, error: &str) {
    assert_eq!(
        response.json(),
        serde_json::json!({
            "code": code,
            "error": error,
        })
    );
}

fn session_spec() -> serde_json::Value {
    serde_json::json!({
        "goal": "Create a CLI --pattern filter command",
        "profile": "python-cli",
        "provider": "ollama",
        "model": "fixture-executor",
        "planner_provider": "ollama",
        "planner_model": "fixture-planner"
    })
}

#[cfg(unix)]
fn write_terminal_cli(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(
        path,
        "#!/bin/sh\nprintf '%s\\n' '{\"event\":\"tui_command_stop\",\"ok\":false,\"status\":\"failed\",\"assurance_level\":\"none\"}' > \"$COMMANDAGENT_EVAL_EVENTS\"\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

#[cfg(unix)]
fn write_pack_capture_cli(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(
        path,
        "#!/bin/sh\nif [ \"${1-}\" = \"--version\" ]; then printf 'commandagent 0.1.0 test\\n'; exit 0; fi\nenv | sort > \"${COMMANDAGENT_EVAL_EVENTS%/*}/delegated-env.txt\"\nprintf '%s\\n' '{\"event\":\"tui_command_stop\",\"ok\":false,\"status\":\"failed\",\"assurance_level\":\"none\"}' > \"$COMMANDAGENT_EVAL_EVENTS\"\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}
