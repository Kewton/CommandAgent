#![cfg(all(feature = "gui", unix))]

//! Issue #544 self-check (L-14): rows 1-17 of H-09 `checks-544.md`.
//!
//! The real `gui_server` handler and projection functions are exercised in
//! process with a fake `AppState` on a current-thread runtime — no server, no
//! socket, no model or API. Every row is a focused test named `r<row>_...` and
//! registers only fake values.

mod gui {
    // The whole server binary is included for its handlers, so most of it is
    // unreferenced from this test crate; the production build uses it all.
    #![allow(dead_code, unused_imports, unused_variables, unused_mut)]

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/gui_server.rs"
    ));

    #[cfg(test)]
    mod focused {
        use std::path::{Path, PathBuf};

        use axum::body::to_bytes;
        use axum::extract::{Path as AxumPath, Query, State};
        use axum::http::{HeaderMap, StatusCode};
        use axum::response::{IntoResponse, Response};
        use commandagent::planner::adjudication::contract::IntentId;
        use commandagent::planner::pack::catalog::PackSource;
        use commandagent::planner::profile::ProfileId;
        use commandagent::sensitive_data::{self, SecretCatalog, set_current};
        use commandagent::tui::boundary_shell::band_catalog::value_for;
        use commandagent::tui::boundary_shell::confirmation::{
            ConfirmationIdentity, DraftManifestIdentity, ExecutionPins, PackSelection,
        };
        use commandagent::tui::boundary_shell::family_catalog::TaskFamilyId;
        use commandagent::tui::boundary_shell::route::{RouteBasis, RouteCandidate};
        use serde_json::{Value, json};
        use sha2::{Digest, Sha256};

        use super::{
            AppState, api, public_projection, session_files, trial_access, trial_process,
            workspace_policy,
        };

        const C1: &str = "H01_CANARY_JwtStyle_NonPrefix_29486";
        const C2: &str = "H09_日本語_Canary_秘密値_5521";
        const C3: &str = "H09_Q\"uo\\te_Canary_7731";
        const C6_LABEL: &str = "xecution-roo";

        fn block_on<F: std::future::Future>(future: F) -> F::Output {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(future)
        }

        fn register(root: &Path, secrets: &[&str]) {
            let mut catalog = SecretCatalog::new();
            for secret in secrets {
                let _ = catalog.register_plain(secret);
            }
            sensitive_data::install_scope(catalog, Some(root), None);
        }

        struct Fixture {
            _root: tempfile::TempDir,
            repository: PathBuf,
            execution: PathBuf,
        }

        fn fixture() -> Fixture {
            let root = tempfile::tempdir().unwrap();
            let repository = root.path().join("repository");
            let execution = root.path().join("execution");
            std::fs::create_dir_all(&repository).unwrap();
            std::fs::create_dir_all(&execution).unwrap();
            Fixture {
                _root: root,
                repository: repository.canonicalize().unwrap(),
                execution: execution.canonicalize().unwrap(),
            }
        }

        fn app_state(fixture: &Fixture) -> AppState {
            AppState {
                repository_root: fixture.repository.clone(),
                static_root: fixture.repository.join("static"),
                base_path: "/".to_string(),
                commandagent_bin: fixture.repository.join("commandagent"),
                ollama_host: "http://localhost:11434".to_string(),
                lm_studio_host: "http://localhost:1234".to_string(),
                extension_root: None,
                trial_access: trial_access::TrialAccess::from_environment(true, false).unwrap(),
                trial_processes: trial_process::TrialProcesses::new(),
                trial_workspace: workspace_policy::TrialWorkspace::configure(
                    &fixture.repository,
                    Some(&fixture.execution),
                )
                .unwrap(),
            }
        }

        fn run_root(fixture: &Fixture, id: &str) -> PathBuf {
            fixture
                .repository
                .join("workspace/management/runs")
                .join(id)
        }

        fn session_root(fixture: &Fixture, id: &str) -> PathBuf {
            fixture.execution.join(".commandagent/runs").join(id)
        }

        fn write_at(path: &Path, content: &str) {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(path, content).unwrap();
        }

        fn sha256(path: &Path) -> String {
            format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
        }

        async fn response_json(response: Response) -> (StatusCode, Value) {
            let status = response.status();
            let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            (
                status,
                serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            )
        }

        fn events_query(tail: usize) -> session_files::EventsQuery {
            serde_json::from_value(json!({ "tail": tail })).unwrap()
        }

        fn artifact_query(path: Option<&str>) -> session_files::ArtifactQuery {
            serde_json::from_value(json!({ "path": path })).unwrap()
        }

        fn identity(workspace: &Path, request: String) -> ConfirmationIdentity {
            let route = RouteCandidate {
                profile: ProfileId::Ingest,
                intent: IntentId::Create,
                family: TaskFamilyId::List,
                bases: vec![RouteBasis {
                    rule: "fixture",
                    observation: "list".to_string(),
                }],
                contract_ref: "docs/ingest-profile-contract.md",
            };
            let mut identity = ConfirmationIdentity::new(
                request,
                workspace,
                &route,
                value_for("ingest", IntentId::Create, TaskFamilyId::List).unwrap(),
                ExecutionPins {
                    planner_provider: "ollama".to_string(),
                    planner_model: "planner".to_string(),
                    executor_provider: "ollama".to_string(),
                    executor_model: "executor".to_string(),
                    preset: "profile".to_string(),
                    think: None,
                },
                PackSelection::None,
            )
            .unwrap();
            identity.route_bases = vec!["rule=observation".to_string()];
            identity
        }

        // Row 1: `public_projection.rs::text` scrubs the secret and replaces the
        // execution root in the same free text.
        #[test]
        fn r01_text_scrubs_secret_and_execution_root() {
            let fixture = fixture();
            register(&fixture.execution, &[C1]);
            let value = format!("log {C1} at {}/sub/dir", fixture.execution.display());
            let projected = public_projection::text(value, &fixture.execution);
            assert!(!projected.contains(C1), "{projected}");
            assert!(
                !projected.contains(fixture.execution.to_string_lossy().as_ref()),
                "{projected}"
            );
            assert!(projected.contains("<execution-root>"), "{projected}");
        }

        // Row 2: a root path that itself contains a secret leaves no fragment,
        // under either replacement order and with the `/private/` alias.
        #[test]
        fn r02_root_path_containing_secret_is_fully_replaced() {
            let root = tempfile::tempdir().unwrap();
            let secret_root = root.path().join(format!("ws-{C1}"));
            std::fs::create_dir_all(&secret_root).unwrap();
            let secret_root = secret_root.canonicalize().unwrap();
            register(&secret_root, &[C1]);

            let projected =
                public_projection::text(format!("at {}", secret_root.display()), &secret_root);
            assert!(!projected.contains(C1), "{projected}");
            assert!(projected.contains("<execution-root>"), "{projected}");

            if let Some(alias) = secret_root.to_string_lossy().strip_prefix("/private/") {
                let projected = public_projection::text(format!("at /{alias}"), &secret_root);
                assert!(!projected.contains(C1), "{projected}");
                assert!(projected.contains("<execution-root>"), "{projected}");
            }
        }

        // Row 3: the fixed `<execution-root>` label is a schema marker; a
        // registered substring of it must not corrupt it.
        #[test]
        fn r03_fixed_execution_root_label_is_not_corrupted() {
            let fixture = fixture();
            register(&fixture.execution, &[C6_LABEL]);
            let projected = public_projection::text(
                format!("at {}", fixture.execution.display()),
                &fixture.execution,
            );
            assert_eq!(projected, "at <execution-root>");
        }

        // Row 4: the identity projection scrubs every field, not just workspace.
        #[test]
        fn r04_identity_projection_scrubs_every_field() {
            let fixture = fixture();
            register(&fixture.execution, &[C1]);
            let mut identity = identity(&fixture.execution, format!("create ingest {C1}"));
            identity.route_bases = vec![format!("rule={C1}")];
            identity.pins.planner_model = format!("model-{C1}");
            identity.pack = PackSelection::Pinned {
                id: format!("pack-{C1}"),
                version: "1.0.0".to_string(),
                hash: format!("sha256:{C1}"),
                point: "point".to_string(),
                source: PackSource::Admitted,
            };
            identity.draft_manifest = Some(DraftManifestIdentity {
                source: format!("src-{C1}"),
                path: format!("path-{C1}"),
                hash: format!("hash-{C1}"),
                assurance_ceiling: format!("ceiling-{C1}"),
                base_profile: Some(format!("base-{C1}")),
            });

            let projected = public_projection::identity(&identity, &fixture.execution);
            let serialized = serde_json::to_string(&projected).unwrap();
            assert!(!serialized.contains(C1), "{serialized}");
            assert!(serialized.contains("<redacted>"), "{serialized}");
            // A display copy never rewrites the hashed source.
            assert!(identity.request.contains(C1));
        }

        // Row 5: a JSON document stays parseable, keeps its key set and the
        // fixed `verdict`/`status` values.
        #[test]
        fn r05_document_redaction_keeps_json_parseable_and_fixed_keys() {
            let fixture = fixture();
            register(&fixture.repository, &[C1, "completed"]);
            let run = run_root(&fixture, "run-544");
            let path = run.join("evidence.json");
            write_at(
                &path,
                &format!("{{\"status\":\"completed\",\"verdict\":\"pass\",\"note\":\"{C1}\"}}"),
            );

            let mut document = block_on(api::document(&run, &path)).unwrap();
            document.redact_execution_root(&fixture.repository);
            let value = serde_json::to_value(&document).unwrap();
            let content = value["content"].as_str().unwrap();
            assert!(!content.contains(C1), "{content}");
            let parsed: Value = serde_json::from_str(content).unwrap();
            assert_eq!(parsed["status"], "completed");
            assert_eq!(parsed["verdict"], "pass");
            assert_eq!(parsed.as_object().unwrap().len(), 3);
        }

        // Row 6: an escaped secret inside a JSON string is scrubbed (a text-only
        // exact match would miss it).
        #[test]
        fn r06_document_json_escaped_secret_is_scrubbed() {
            let fixture = fixture();
            register(&fixture.repository, &[C3]);
            let run = run_root(&fixture, "run-544");
            let path = run.join("evidence.json");
            let body = json!({"event": "x", "note": C3}).to_string();
            assert!(body.contains("\\\""), "fixture must be escaped: {body}");
            write_at(&path, &body);

            let mut document = block_on(api::document(&run, &path)).unwrap();
            document.redact_execution_root(&fixture.repository);
            let value = serde_json::to_value(&document).unwrap();
            let content = value["content"].as_str().unwrap();
            assert!(!content.contains(C3), "{content}");
            let parsed: Value = serde_json::from_str(content).unwrap();
            assert_eq!(parsed["note"], "<redacted>");
        }

        // Row 7: a fixed evidence-envelope schema key keeps its value.
        #[test]
        fn r07_evidence_envelope_fixed_schema_is_kept() {
            let fixture = fixture();
            register(&fixture.repository, &["ool_parse_fail"]);
            let run = run_root(&fixture, "run-544");
            let path = run.join("evidence.json");
            write_at(
                &path,
                r#"{"event":"x","evidence_envelope":{"envelope_version":1,"family":"tool_parse","kind":"tool_parse_failure","source_refs":["note"]}}"#,
            );

            let mut document = block_on(api::document(&run, &path)).unwrap();
            document.redact_execution_root(&fixture.repository);
            let value = serde_json::to_value(&document).unwrap();
            let parsed: Value = serde_json::from_str(value["content"].as_str().unwrap()).unwrap();
            assert_eq!(parsed["evidence_envelope"]["kind"], "tool_parse_failure");
            assert_eq!(parsed["evidence_envelope"]["family"], "tool_parse");
            assert_eq!(parsed["evidence_envelope"]["envelope_version"], 1);
        }

        // Row 8 (linked to #543 row 2): a list id/path carrying the secret,
        // including a `build-verifier-<slug>.log` name, is scrubbed.
        #[test]
        fn r08_document_summary_scrubs_filename_and_path() {
            let fixture = fixture();
            register(&fixture.repository, &[C1]);
            let run = run_root(&fixture, &format!("run-{C1}"));
            let log = run.join(format!("build-verifier-{C1}.log"));
            write_at(&log, "log line\n");

            let summary = api::document_summary(&run, &fixture.repository, &log).unwrap();
            let value = serde_json::to_value(&summary).unwrap();
            assert!(!value.to_string().contains(C1), "{value}");
            assert!(
                value["path"]
                    .as_str()
                    .unwrap()
                    .contains("build-verifier-<redacted>.log")
            );

            let index = block_on(api::runs(State(app_state(&fixture)))).unwrap();
            let value = serde_json::to_value(index.0).unwrap();
            assert!(!value.to_string().contains(C1), "{value}");
            assert!(
                value["runs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|run| run["id"].as_str().unwrap() == "run-<redacted>")
            );
        }

        // Row 9: a #504-era raw record is scrubbed on the way out and the file
        // is not written back.
        #[test]
        fn r09_run_list_and_detail_do_not_write_back_legacy_record() {
            let fixture = fixture();
            register(&fixture.repository, &[C1]);
            let run = run_root(&fixture, "run-544");
            let acceptance = run.join("acceptance-sheet.md");
            write_at(
                &acceptance,
                &format!("# Acceptance\n\nStatus: failed token={C1}\n"),
            );
            let before = sha256(&acceptance);

            let state = app_state(&fixture);
            let index = block_on(api::runs(State(state.clone()))).unwrap();
            assert!(
                !serde_json::to_value(index.0)
                    .unwrap()
                    .to_string()
                    .contains(C1)
            );
            let detail = block_on(api::run_detail(
                State(state),
                AxumPath("run-544".to_string()),
            ))
            .unwrap();
            assert!(
                !serde_json::to_value(detail.0)
                    .unwrap()
                    .to_string()
                    .contains(C1)
            );
            assert_eq!(sha256(&acceptance), before);
        }

        // Row 10: a JSONL event tail with an escaped secret is scrubbed per line.
        #[test]
        fn r10_event_tail_scrubs_json_escaped_secret() {
            let fixture = fixture();
            register(&fixture.execution, &[C3]);
            let id = uuid::Uuid::from_u128(0x544).to_string();
            let path = session_root(&fixture, &id).join("events.jsonl");
            write_at(
                &path,
                &format!("{}\n", json!({"event": "run_start", "note": C3})),
            );

            let response = block_on(session_files::events(
                State(app_state(&fixture)),
                AxumPath(id),
                HeaderMap::new(),
                Query(events_query(100)),
            ))
            .unwrap();
            let (status, value) = block_on(response_json(response));
            assert_eq!(status, 200);
            let content = value["content"].as_str().unwrap();
            assert!(!content.contains(C3), "{content}");
            assert_eq!(content.lines().count(), 1);
            let parsed: Value = serde_json::from_str(content.trim_end()).unwrap();
            assert_eq!(parsed["note"], "<redacted>");
        }

        // Row 11: two dynamic keys that carry secrets do not duplicate a key,
        // do not change the element count and do not rename an unrelated key.
        #[test]
        fn r11_event_tail_dynamic_keys_do_not_duplicate_or_rename() {
            let fixture = fixture();
            register(&fixture.execution, &[C1, C2]);
            let id = uuid::Uuid::from_u128(0x545).to_string();
            let path = session_root(&fixture, &id).join("events.jsonl");
            write_at(
                &path,
                &format!("{{\"event\":\"x\",\"{C1}\":1,\"{C2}\":2,\"<redacted>\":3,\"keep\":4}}\n"),
            );

            let response = block_on(session_files::events(
                State(app_state(&fixture)),
                AxumPath(id),
                HeaderMap::new(),
                Query(events_query(100)),
            ))
            .unwrap();
            let (status, value) = block_on(response_json(response));
            assert_eq!(status, 200);
            let content = value["content"].as_str().unwrap();
            assert!(!content.contains(C1) && !content.contains(C2), "{content}");
            let parsed: Value = serde_json::from_str(content.trim_end()).unwrap();
            let map = parsed.as_object().unwrap();
            assert_eq!(map.len(), 5, "{content}");
            assert!(
                map.contains_key("event") && map.contains_key("keep"),
                "{content}"
            );
            assert!(map.contains_key("<redacted>"), "{content}");
            assert!(
                map.keys()
                    .all(|key| !key.contains("H01") && !key.contains("H09")),
                "{content}"
            );
        }

        // Row 12: a fixed schema key name is never renamed, and the line count
        // and order are preserved.
        #[test]
        fn r12_event_tail_fixed_names_are_kept() {
            let fixture = fixture();
            register(&fixture.execution, &["schema_v"]);
            let id = uuid::Uuid::from_u128(0x546).to_string();
            let path = session_root(&fixture, &id).join("events.jsonl");
            write_at(
                &path,
                "{\"event\":\"run_start\",\"status\":\"completed\",\"verdict\":\"pass\",\"schema_version\":\"commandagent.confirmation/v1\"}\n{\"event\":\"run_stop\",\"status\":\"completed\"}\n",
            );

            let response = block_on(session_files::events(
                State(app_state(&fixture)),
                AxumPath(id),
                HeaderMap::new(),
                Query(events_query(100)),
            ))
            .unwrap();
            let (status, value) = block_on(response_json(response));
            assert_eq!(status, 200);
            let content = value["content"].as_str().unwrap();
            let lines = content.lines().collect::<Vec<_>>();
            assert_eq!(lines.len(), 2);
            let first: Value = serde_json::from_str(lines[0]).unwrap();
            for key in ["event", "status", "verdict", "schema_version"] {
                assert!(first.as_object().unwrap().contains_key(key), "{content}");
            }
            assert_eq!(first["event"], "run_start");
            assert!(content.contains("\"event\":\"run_stop\""), "{content}");
        }

        // Row 13: a secret in the free text of a root fixed key is scrubbed,
        // while the legacy record keeps its bytes.
        #[test]
        fn r13_event_tail_root_fixed_key_free_text_is_scrubbed() {
            let fixture = fixture();
            register(&fixture.execution, &[C1]);
            let id = uuid::Uuid::from_u128(0x547).to_string();
            let path = session_root(&fixture, &id).join("events.jsonl");
            write_at(
                &path,
                &format!("{{\"event\":\"run_start\",\"status\":\"failed token={C1}\"}}\n"),
            );

            let response = block_on(session_files::events(
                State(app_state(&fixture)),
                AxumPath(id),
                HeaderMap::new(),
                Query(events_query(100)),
            ))
            .unwrap();
            let (status, value) = block_on(response_json(response));
            assert_eq!(status, 200);
            let content = value["content"].as_str().unwrap();
            assert!(!content.contains(C1), "{content}");
            let parsed: Value = serde_json::from_str(content.trim_end()).unwrap();
            assert_eq!(parsed["event"], "run_start");
            assert!(
                parsed["status"].as_str().unwrap().contains("<redacted>"),
                "{content}"
            );
            assert!(std::fs::read_to_string(&path).unwrap().contains(C1));
        }

        // Row 14: a tail boundary that excludes the secret line leaves no
        // fragment of it.
        #[test]
        fn r14_event_tail_truncation_does_not_leak_prefix() {
            let fixture = fixture();
            register(&fixture.execution, &[C1]);
            let id = uuid::Uuid::from_u128(0x548).to_string();
            let path = session_root(&fixture, &id).join("events.jsonl");
            write_at(
                &path,
                &format!(
                    "{{\"event\":\"run_start\",\"note\":\"{C1}\"}}\n{{\"event\":\"middle\"}}\n{{\"event\":\"run_stop\"}}\n"
                ),
            );

            let response = block_on(session_files::events(
                State(app_state(&fixture)),
                AxumPath(id),
                HeaderMap::new(),
                Query(events_query(2)),
            ))
            .unwrap();
            let (status, value) = block_on(response_json(response));
            assert_eq!(status, 200);
            let content = value["content"].as_str().unwrap();
            assert!(!content.contains(C1), "{content}");
            assert!(!content.contains("H01_CANARY"), "{content}");
            assert_eq!(content.lines().count(), 2);
        }

        // Row 15: an artifact body and the artifact list are scrubbed, and the
        // source bytes are unchanged.
        #[test]
        fn r15_artifacts_body_and_list_are_scrubbed_without_writeback() {
            let fixture = fixture();
            register(&fixture.execution, &[C1, C2]);
            let id = uuid::Uuid::from_u128(0x549).to_string();
            let artifact = session_root(&fixture, &id).join("note.json");
            write_at(&artifact, &format!("{{\"note\":\"{C1} {C2}\"}}"));
            let before = sha256(&artifact);

            let state = app_state(&fixture);
            let response = block_on(session_files::artifacts(
                State(state.clone()),
                AxumPath(id.clone()),
                HeaderMap::new(),
                Query(artifact_query(None)),
            ))
            .unwrap();
            let (status, list) = block_on(response_json(response));
            assert_eq!(status, 200);
            assert!(!list.to_string().contains(C1), "{list}");

            let response = block_on(session_files::artifacts(
                State(state),
                AxumPath(id),
                HeaderMap::new(),
                Query(artifact_query(Some("note.json"))),
            ))
            .unwrap();
            let (status, document) = block_on(response_json(response));
            assert_eq!(status, 200);
            let content = document["content"].as_str().unwrap();
            assert!(!content.contains(C1) && !content.contains(C2), "{content}");
            assert_eq!(sha256(&artifact), before);
        }

        // Row 16: two workspaces keep their own catalog inside a tokio task,
        // even with no thread-local scope; neither value crosses over.
        #[test]
        fn r16_two_workspaces_do_not_share_catalog_in_blocking_task() {
            let alpha = fixture();
            let beta = fixture();
            let alpha_secret = "AAA_A_only_secret_value";
            let beta_secret = "BBB_B_only_secret_value";
            register(&alpha.execution, &[alpha_secret]);
            register(&beta.execution, &[beta_secret]);
            set_current(None);

            let alpha_id = uuid::Uuid::from_u128(0x54a).to_string();
            let beta_id = uuid::Uuid::from_u128(0x54b).to_string();
            let line =
                format!("{{\"event\":\"run_start\",\"note\":\"{alpha_secret} {beta_secret}\"}}\n");
            write_at(&session_root(&alpha, &alpha_id).join("events.jsonl"), &line);
            write_at(&session_root(&beta, &beta_id).join("events.jsonl"), &line);

            let content_a = block_on(async move {
                tokio::spawn(fetch_events(app_state(&alpha), alpha_id))
                    .await
                    .unwrap()
            });
            let content_b = block_on(async move {
                tokio::spawn(fetch_events(app_state(&beta), beta_id))
                    .await
                    .unwrap()
            });

            assert!(!content_a.contains(alpha_secret), "{content_a}");
            assert!(content_a.contains(beta_secret), "{content_a}");
            assert!(!content_b.contains(beta_secret), "{content_b}");
            assert!(content_b.contains(alpha_secret), "{content_b}");
        }

        async fn fetch_events(state: AppState, id: String) -> String {
            let response = session_files::events(
                State(state),
                AxumPath(id),
                HeaderMap::new(),
                Query(events_query(100)),
            )
            .await
            .unwrap();
            let (status, value) = response_json(response).await;
            assert_eq!(status, 200);
            value["content"].as_str().unwrap().to_string()
        }

        // Row 17: a body that cannot be projected safely fails honestly; a
        // fixed-name collision is preserved rather than corrupted.
        #[test]
        fn r17_unsafe_projection_fails_honestly() {
            let fixture = fixture();
            register(&fixture.repository, &[C1, "completed"]);
            let run = run_root(&fixture, "run-544");
            write_at(
                &run.join("refused.json"),
                &format!("{{\"event\":\"x\",\"{C1}\":1}}"),
            );
            write_at(
                &run.join("completed.json"),
                "{\"event\":\"x\",\"status\":\"completed\",\"note\":\"ok\"}",
            );

            let state = app_state(&fixture);
            let refused: api::EvidenceQuery =
                serde_json::from_value(json!({"path": "refused.json"})).unwrap();
            let error = block_on(api::run_evidence(
                State(state.clone()),
                AxumPath("run-544".to_string()),
                Query(refused),
            ))
            .unwrap_err();
            let (status, value) = block_on(response_json(error.into_response()));
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(value["code"], "secret_projection_refused");
            assert!(!value.to_string().contains(C1), "{value}");

            let completed: api::EvidenceQuery =
                serde_json::from_value(json!({"path": "completed.json"})).unwrap();
            let document = block_on(api::run_evidence(
                State(state),
                AxumPath("run-544".to_string()),
                Query(completed),
            ))
            .unwrap();
            let content = serde_json::to_value(&document.0).unwrap()["content"]
                .as_str()
                .unwrap()
                .to_string();
            let parsed: Value = serde_json::from_str(&content).unwrap();
            // The fixed schema key names are preserved (no rename, no
            // corruption); how a fixed key's *value* is treated is asserted by
            // `b544_1_evidence_document_scrubs_fixed_key_values`.
            let map = parsed.as_object().unwrap();
            assert_eq!(map.len(), 3);
            assert!(map.contains_key("status"));
            assert_eq!(parsed["event"], "x");
        }

        // B-544-1 (H-10): a document body whose root fixed key holds free text
        // must scrub that value. The catalog is the real GUI one built from the
        // repository root `.env` (no `install_scope`), and the legacy file is
        // not rewritten.
        #[test]
        fn b544_1_evidence_document_scrubs_fixed_key_values() {
            set_current(None);
            let fixture = fixture();
            std::fs::write(
                fixture.repository.join(".env"),
                format!("PRIVATE_TOKEN={C1}\n"),
            )
            .unwrap();
            let run = run_root(&fixture, "run-h10");
            let acceptance = run.join("acceptance.json");
            write_at(
                &acceptance,
                &json!({
                    "event": "acceptance",
                    "status": format!("failed token={C1}"),
                    "verdict": "fail",
                    "tool_name": C1,
                    "note": C1,
                    "nested": {"status": C1},
                })
                .to_string(),
            );
            write_at(
                &run.join("summary.md"),
                &format!("Status: failed\nReason: {C1}\n"),
            );
            let before = sha256(&acceptance);

            let state = app_state(&fixture);
            let query: api::EvidenceQuery =
                serde_json::from_value(json!({"path": "acceptance.json"})).unwrap();
            let document = block_on(api::run_evidence(
                State(state.clone()),
                AxumPath("run-h10".to_string()),
                Query(query),
            ))
            .unwrap();
            let content = serde_json::to_value(&document.0).unwrap()["content"]
                .as_str()
                .unwrap()
                .to_string();
            assert!(!content.contains(C1), "{content}");
            assert!(!content.contains("H01_CANARY"), "{content}");
            let parsed: Value = serde_json::from_str(&content).unwrap();
            let map = parsed.as_object().unwrap();
            assert_eq!(map.len(), 6, "{content}");
            for key in ["event", "status", "verdict", "tool_name", "note", "nested"] {
                assert!(map.contains_key(key), "{content}");
            }
            assert_eq!(parsed["event"], "acceptance");
            assert_eq!(parsed["status"], "failed token=<redacted>");
            assert_eq!(parsed["tool_name"], "<redacted>");
            assert_eq!(parsed["nested"]["status"], "<redacted>");

            // The text document and the run index stay clean too.
            let summary: api::EvidenceQuery =
                serde_json::from_value(json!({"path": "summary.md"})).unwrap();
            let summary_document = block_on(api::run_evidence(
                State(state.clone()),
                AxumPath("run-h10".to_string()),
                Query(summary),
            ))
            .unwrap();
            assert!(
                !serde_json::to_value(&summary_document.0)
                    .unwrap()
                    .to_string()
                    .contains(C1)
            );
            let index = block_on(api::runs(State(state))).unwrap();
            assert!(
                !serde_json::to_value(index.0)
                    .unwrap()
                    .to_string()
                    .contains(C1)
            );

            // The legacy record is not rewritten.
            assert_eq!(sha256(&acceptance), before);
        }
    }
}
