use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
struct ProtectionRule {
    category: &'static str,
    site_predicate: &'static str,
    required_wrapper: &'static str,
    allowlist: &'static [&'static str],
    audit: fn(&AuditCorpus, &ProtectionRule) -> Vec<String>,
}

struct AuditCorpus {
    files: BTreeMap<String, String>,
}

const PROTECTION_RULES: &[ProtectionRule] = &[
    ProtectionRule {
        category: "compile_output_source_of_truth",
        site_predicate: "compile diagnostic parsers",
        required_wrapper: "FullCommandOutput",
        allowlist: &[
            "src/minimal_loop/build_verifier.rs",
            "src/planner/profile.rs",
            "src/planner/profiles/manifest_driven.rs",
            "src/planner/profiles/python_cli.rs",
            "src/planner/fix_diagnostics.rs",
            "src/planner/runner/acceptance.rs",
        ],
        audit: audit_compile_output_source_of_truth,
    },
    ProtectionRule {
        category: "verify_normalization_boundary",
        site_predicate: "executes-or-policy-checks-model-commands",
        required_wrapper: "NormalizedVerifyCommand",
        allowlist: &[],
        audit: audit_verify_normalization_boundary,
    },
    ProtectionRule {
        category: "bounded_execution_chokepoints",
        site_predicate: "provider chat and child process launch",
        required_wrapper: "provider_call / bounded_process / confirmed GUI CLI delegate",
        allowlist: &[
            "src/provider_call.rs",
            "src/bounded_process.rs",
            "src/bin/gui_server/delegate.rs",
        ],
        audit: audit_bounded_execution_chokepoints,
    },
    ProtectionRule {
        category: "fetch_network_boundary",
        site_predicate: "network fetch transport and dispatch",
        required_wrapper: "fetch_probe / bounded_process",
        allowlist: &["src/fetch_probe/mod.rs", "src/fetch_probe/transport.rs"],
        audit: audit_fetch_network_boundary,
    },
    ProtectionRule {
        category: "provider_timeout_enforcement",
        site_predicate: "ChatClient implementors",
        required_wrapper: "required boxed_clone worker path",
        allowlist: &["src/providers/mod.rs", "src/provider_call.rs"],
        audit: audit_provider_timeout_enforcement,
    },
    ProtectionRule {
        category: "workspace_policy_tool_paths",
        site_predicate: "ToolRegistry path arguments",
        required_wrapper: "resolve_policy_checked_path / ensure_tool_path_allowed",
        allowlist: &["src/tools/registry.rs"],
        audit: audit_workspace_policy_tool_paths,
    },
    ProtectionRule {
        category: "terminal_records",
        site_predicate: "CLI command exit paths",
        required_wrapper: "DirectCommandCompletionGuard / tui_command_stop",
        allowlist: &["src/lib.rs", "tests/conformance/mod.rs"],
        audit: audit_terminal_records,
    },
    ProtectionRule {
        category: "extension_supply_writes",
        site_predicate: "writes below an extension_root",
        required_wrapper: "planner::pack::supply::SupplyRoot",
        allowlist: &["src/planner/pack/supply.rs"],
        audit: audit_extension_supply_writes,
    },
];

#[test]
fn protection_coverage_table_is_green_for_current_tree() {
    let corpus = AuditCorpus::from_manifest_dir();
    let violations = audit_protection_coverage(&corpus);
    assert!(
        violations.is_empty(),
        "protection coverage audit failed:\n{}",
        violations.join("\n")
    );
}

#[test]
fn protection_coverage_table_rejects_unregistered_mock_sites() {
    let corpus = AuditCorpus::from_pairs([
        (
            "src/new_provider.rs",
            r#"fn f(client: &mut dyn Client) { let _ = client.chat("m", &[], &[], false); }"#,
        ),
        (
            "src/providers/mod.rs",
            r#"pub trait ChatClient { fn boxed_clone(&self) -> Option<Box<dyn ChatClient>> { None } }"#,
        ),
        (
            "src/provider_call.rs",
            r#"fn f(client: &mut dyn ChatClient) { let Some(mut worker_client) = client.boxed_clone() else { return client.chat("m", &[], &[], false); }; }"#,
        ),
        (
            "src/new_spawn.rs",
            r#"fn f() { let _ = std::process::Command::new("sh").output(); }"#,
        ),
        (
            "src/bin/gui_server/sessions.rs",
            r#"fn f() { let _ = std::process::Command::new("commandagent").spawn(); }"#,
        ),
        (
            "src/new_fetch.rs",
            r#"fn f() { let _ = std::process::Command::new("curl").arg("https://example.test"); let _ = transport.get(root, request); }"#,
        ),
        (
            "src/new_verify.rs",
            r#"fn f(root: &std::path::Path) { let _ = crate::minimal_loop::verifier_env::run_checked("npm run build", root, false); }"#,
        ),
        (
            "src/new_runtime_bash.rs",
            r#"fn runtime_bash_policy_decision(command: &str) { let _ = crate::planner::verify::diagnose_verify_command(command); }"#,
        ),
        (
            "src/planner/lint.rs",
            r#"fn lint(command: &str) { let _ = crate::planner::verify::validate_verify_command(command); }"#,
        ),
        (
            "src/new_compile.rs",
            r#"fn f(excerpt: &str) { let _ = crate::minimal_loop::build_verifier::parse_compile_errors(excerpt); }"#,
        ),
        (
            "src/tools/registry.rs",
            r#"fn default_tool_specs() { spec("Read", "", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]})); spec("Write", "", json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]})); } fn execute() { let _ = resolve_existing(&context.root, raw); }"#,
        ),
        (
            "src/config.rs",
            "pub enum Action { Repl, Prompt(String), NewEntry(String) }",
        ),
        (
            "src/lib.rs",
            r#"fn run() {} fn direct_command_for_action(action: &Action) -> Option<&'static str> { match action { Action::Repl => None, Action::Prompt(_) => Some("--prompt") } }"#,
        ),
        (
            "src/new_extension_writer.rs",
            r#"fn f(extension_root: &Path) { std::fs::write(extension_root.join("packs/x"), b"x").unwrap(); }"#,
        ),
    ]);
    let violations = audit_protection_coverage(&corpus).join("\n");
    for category in [
        "compile_output_source_of_truth",
        "verify_normalization_boundary",
        "bounded_execution_chokepoints",
        "fetch_network_boundary",
        "provider_timeout_enforcement",
        "workspace_policy_tool_paths",
        "terminal_records",
        "extension_supply_writes",
    ] {
        assert!(
            violations.contains(category),
            "mock site did not trip {category}; violations:\n{violations}"
        );
    }
    assert!(
        violations.contains("src/new_fetch.rs")
            && violations.contains("constructs a fetch child outside the boundary"),
        "mock fetch site did not trip the single-boundary guard: {violations}"
    );
    assert!(
        violations.contains("src/bin/gui_server/sessions.rs")
            && violations.contains("direct child-process invocation"),
        "old GUI process location did not trip the moved delegate guard: {violations}"
    );
}

fn audit_protection_coverage(corpus: &AuditCorpus) -> Vec<String> {
    let mut violations = Vec::new();
    for rule in PROTECTION_RULES {
        for violation in (rule.audit)(corpus, rule) {
            violations.push(format!(
                "{} [{} -> {}]: {}",
                rule.category, rule.site_predicate, rule.required_wrapper, violation
            ));
        }
    }
    violations
}

fn audit_compile_output_source_of_truth(
    corpus: &AuditCorpus,
    rule: &ProtectionRule,
) -> Vec<String> {
    let mut violations = Vec::new();
    let build_verifier = corpus.file("src/minimal_loop/build_verifier.rs");
    if !build_verifier.contains("pub fn parse_compile_errors(output: &FullCommandOutput)") {
        violations.push("parse_compile_errors must require &FullCommandOutput".to_string());
    }
    if build_verifier.contains("pub fn parse_compile_errors(output: &str)") {
        violations.push("parse_compile_errors still accepts a raw string".to_string());
    }
    if corpus
        .file("src/planner/runner/acceptance.rs")
        .contains("release_evidence_compile_excerpt_fields")
    {
        violations.push("release evidence still has an excerpt-to-parser fallback".to_string());
    }
    for (path, text) in corpus.rust_files() {
        for (line_index, line) in text.lines().enumerate() {
            if line.contains("parse_compile_errors(")
                && !rule.allowlist.contains(&path.as_str())
                && !line.trim_start().starts_with("///")
            {
                violations.push(format!("{path}:{}: {}", line_index + 1, line.trim()));
            }
            if line.contains("parse_compile_errors(") && line.contains("output_excerpt") {
                violations.push(format!(
                    "{path}:{} parses an output excerpt",
                    line_index + 1
                ));
            }
        }
    }
    violations
}

fn audit_verify_normalization_boundary(corpus: &AuditCorpus, _: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    for (path, text) in corpus.rust_files() {
        if path == "src/planner/sanitizer.rs" {
            continue;
        }
        let has_shared_verify_normalizer = text.contains("normalize_planner_verify_command(")
            || text.contains("normalize_runtime_bash_command_for_boundary(");
        let has_normalized_verify_command = text.contains("NormalizedVerifyCommand");
        for (line_index, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("///") {
                continue;
            }
            if line.contains("validate_verify_command(")
                && !(has_shared_verify_normalizer || has_normalized_verify_command)
            {
                violations.push(format!(
                    "{path}:{} validate_verify_command call bypasses the shared normalizer",
                    line_index + 1
                ));
            }
            if line.contains("diagnose_verify_command(")
                && !(has_shared_verify_normalizer || has_normalized_verify_command)
            {
                violations.push(format!(
                    "{path}:{} diagnose_verify_command call bypasses the shared normalizer",
                    line_index + 1
                ));
            }
            let calls_executor = line.contains("verifier_env::run_checked(")
                || line.contains("run_structured_for_verify_with_profile(");
            if calls_executor && !has_normalized_verify_command {
                violations.push(format!(
                    "{path}:{}: verifier execution boundary is missing NormalizedVerifyCommand",
                    line_index + 1
                ));
            }
            if line.contains("runtime_bash_policy_decision(")
                && !line.contains("normalize_runtime_bash_command_for_boundary(")
                && !text.contains("normalize_runtime_bash_command_for_boundary(")
            {
                violations.push(format!(
                    "{path}:{} runtime bash policy entry point bypasses the shared normalizer",
                    line_index + 1
                ));
            }
        }
    }
    violations
}

fn audit_bounded_execution_chokepoints(corpus: &AuditCorpus, rule: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    for (path, text) in corpus.src_rust_files() {
        if path.starts_with("src/planner/runner/tests/") {
            continue;
        }
        let mut in_test_mod = false;
        for (line_index, line) in text.lines().enumerate() {
            if path != "src/provider_call.rs" && line.contains(".chat(") {
                violations.push(format!(
                    "{path}:{} direct provider .chat call",
                    line_index + 1
                ));
            }
            if !rule.allowlist.contains(&path.as_str())
                && raw_process_invocation(line)
                && !in_test_mod
            {
                violations.push(format!(
                    "{path}:{} direct child-process invocation: {}",
                    line_index + 1,
                    line.trim()
                ));
            }
            let trimmed = line.trim();
            if (trimmed.starts_with("mod tests") || trimmed.starts_with("pub mod tests"))
                && line.contains('{')
                && preceding_cfg_test(text, line_index)
            {
                in_test_mod = true;
            }
        }
    }
    violations
}

fn audit_fetch_network_boundary(corpus: &AuditCorpus, rule: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    let transport = corpus.file("src/fetch_probe/transport.rs");
    if !transport.contains("impl FetchTransport for BoundedCurlTransport") {
        violations.push("bounded curl transport implementation is missing".to_string());
    }
    if !transport.contains("bounded_process::run_with_timeout(") {
        violations.push("fetch transport bypasses bounded_process".to_string());
    }
    if transport.contains("reqwest::") {
        violations.push("fetch transport uses an in-process network client".to_string());
    }
    for (path, text) in corpus.src_rust_files() {
        let mut in_test_mod = false;
        for (line_index, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if (trimmed.starts_with("mod tests") || trimmed.starts_with("pub mod tests"))
                && line.contains('{')
                && preceding_cfg_test(text, line_index)
            {
                in_test_mod = true;
            }
            if in_test_mod {
                continue;
            }
            let constructs_fetch_child =
                line.contains("Command::new(\"curl\"") || line.contains("Command::new(\"wget\"");
            if constructs_fetch_child && path != "src/fetch_probe/transport.rs" {
                violations.push(format!(
                    "{path}:{} constructs a fetch child outside the boundary",
                    line_index + 1
                ));
            }
            if line.contains("transport.get(") && path != "src/fetch_probe/mod.rs" {
                violations.push(format!(
                    "{path}:{} dispatches fetch transport outside the boundary",
                    line_index + 1
                ));
            }
        }
    }
    for path in rule.allowlist {
        if corpus.file(path).is_empty() {
            violations.push(format!("registered fetch boundary file is missing: {path}"));
        }
    }
    let registry = corpus.file("src/tools/registry.rs");
    if registry.contains("spec(\"Fetch\"") || registry.contains("spec(\"Curl\"") {
        violations.push("LLM tool registry exposes a raw network fetch tool".to_string());
    }
    violations
}

fn audit_provider_timeout_enforcement(corpus: &AuditCorpus, _: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    let providers = corpus.file("src/providers/mod.rs");
    if !providers.contains("fn boxed_clone(&self) -> Box<dyn ChatClient>;") {
        violations.push("ChatClient::boxed_clone must be a required Box return".to_string());
    }
    if providers.contains("fn boxed_clone(&self) -> Option")
        || providers.contains("boxed_clone(&self) -> Option")
    {
        violations.push("ChatClient::boxed_clone is optional".to_string());
    }
    let provider_call = corpus.file("src/provider_call.rs");
    for forbidden in [
        "let Some(mut worker_client) = client.boxed_clone() else",
        "client.chat(model, messages, tools, native_tools_enabled)",
    ] {
        if provider_call.contains(forbidden) {
            violations.push(format!(
                "provider_call contains synchronous timeout fallback marker `{forbidden}`"
            ));
        }
    }
    violations
}

fn audit_workspace_policy_tool_paths(corpus: &AuditCorpus, _: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    let registry = corpus.file("src/tools/registry.rs");
    if registry.is_empty() {
        violations.push("src/tools/registry.rs is missing from audit corpus".to_string());
        return violations;
    }
    if !registry.contains("fn resolve_policy_checked_path(") {
        violations.push("missing resolve_policy_checked_path helper".to_string());
    }
    for tool in registry_path_argument_tools(registry) {
        if !contains_policy_checked_tool_call(registry, &tool) {
            violations.push(format!(
                "ToolRegistry `{tool}` path argument does not use resolve_policy_checked_path"
            ));
        }
    }
    let helper = function_body(registry, "resolve_policy_checked_path").unwrap_or_default();
    if !helper.contains("ensure_tool_path_allowed(") {
        violations
            .push("resolve_policy_checked_path does not call ensure_tool_path_allowed".to_string());
    }
    let registry_without_helper = registry.replace(&helper, "");
    for raw_resolver in [
        "resolve_existing(&context.root",
        "resolve_for_create(&context.root",
        "resolve_optional_existing(&context.root",
    ] {
        if registry_without_helper.contains(raw_resolver) {
            violations.push(format!(
                "ToolRegistry path resolver bypasses policy helper: {raw_resolver}"
            ));
        }
    }
    violations
}

fn audit_terminal_records(corpus: &AuditCorpus, _: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    let lib = corpus.file("src/lib.rs");
    for required in [
        "DirectCommandCompletionGuard::start(&config)",
        "emit_direct_command_stop_with_status",
        "\"event\": \"tui_command_stop\"",
        "\"build_commit\": build_info::COMMIT",
        "\"build_timestamp\": build_info::TIMESTAMP",
    ] {
        if !lib.contains(required) {
            violations.push(format!("missing terminal guard marker `{required}`"));
        }
    }
    for variant in action_variants(corpus.file("src/config.rs")) {
        if !lib.contains(&format!("Action::{variant}")) {
            violations.push(format!(
                "Action::{variant} is not registered in direct_command_for_action"
            ));
        }
    }
    if !corpus
        .file("tests/conformance/mod.rs")
        .contains("conformance_honest_terminal_covers_simulated_panic_exit")
    {
        violations.push("terminal panic exit conformance pathway is missing".to_string());
    }
    violations
}

fn audit_extension_supply_writes(corpus: &AuditCorpus, rule: &ProtectionRule) -> Vec<String> {
    let mut violations = Vec::new();
    let boundary = corpus.file("src/planner/pack/supply.rs");
    for required in ["pub struct SupplyRoot", "pub mod journal", "pub fn append("] {
        if !boundary.contains(required) {
            violations.push(format!("extension supply boundary is missing `{required}`"));
        }
    }
    for (path, text) in corpus.src_rust_files() {
        if rule.allowlist.contains(&path.as_str()) {
            continue;
        }
        let lines = text.lines().collect::<Vec<_>>();
        for index in 0..lines.len() {
            let start = index.saturating_sub(2);
            let end = (index + 3).min(lines.len());
            let window = lines[start..end].join(" ");
            if window.contains("extension_root")
                && [
                    "std::fs::write(",
                    "fs::write(",
                    "create_dir",
                    "OpenOptions",
                    "File::create",
                    "rename(",
                    "remove_file(",
                    "remove_dir",
                ]
                .iter()
                .any(|token| lines[index].contains(token))
            {
                violations.push(format!(
                    "{path}:{} writes through extension_root outside SupplyRoot: {}",
                    index + 1,
                    lines[index].trim()
                ));
            }
        }
    }
    violations
}

fn action_variants(config_source: &str) -> Vec<String> {
    let Some(start) = config_source.find("pub enum Action") else {
        return Vec::new();
    };
    let Some(open) = config_source[start..]
        .find('{')
        .map(|index| start + index + 1)
    else {
        return Vec::new();
    };
    let Some(close) = config_source[open..].find('}').map(|index| open + index) else {
        return Vec::new();
    };
    config_source[open..close]
        .lines()
        .filter_map(|line| {
            let token = line
                .trim()
                .trim_end_matches(',')
                .split(['(', '{', ' '])
                .next()
                .unwrap_or_default();
            (!token.is_empty()).then(|| token.to_string())
        })
        .collect()
}

fn registry_path_argument_tools(registry: &str) -> Vec<String> {
    let mut tools = Vec::new();
    let mut rest = registry;
    while let Some(index) = rest.find("spec(") {
        rest = &rest[index + "spec(".len()..];
        let Some(end) = rest.find("),") else {
            break;
        };
        let call = &rest[..end];
        if call.contains(r#""path""#)
            && let Some(tool) = first_string_literal(call)
        {
            tools.push(tool);
        }
        rest = &rest[end + 2..];
    }
    tools
}

fn contains_policy_checked_tool_call(registry: &str, tool: &str) -> bool {
    let lines = registry.lines().collect::<Vec<_>>();
    lines.iter().enumerate().any(|(index, line)| {
        if !line.contains("resolve_policy_checked_path(") {
            return false;
        }
        let call = lines
            .iter()
            .skip(index)
            .take(6)
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        call.contains(&format!("\"{tool}\""))
    })
}

fn first_string_literal(value: &str) -> Option<String> {
    let start = value.find('"')? + 1;
    let end = value[start..].find('"')? + start;
    Some(value[start..end].to_string())
}

fn function_body(source: &str, name: &str) -> Option<String> {
    let start = source.find(&format!("fn {name}("))?;
    let body_start = source[start..].find('{')? + start;
    let mut depth = 0usize;
    for (offset, ch) in source[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = body_start + offset + 1;
                    return Some(source[start..end].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn raw_process_invocation(line: &str) -> bool {
    if line.contains(".spawn(") && !line.contains(".spawn(move") {
        return true;
    }
    if line.contains(".output(") {
        return true;
    }
    line.contains(".status()") && !line.contains("response.status()")
}

fn preceding_cfg_test(text: &str, line_index: usize) -> bool {
    text.lines()
        .take(line_index)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .take_while(|line| line.trim().is_empty() || line.trim().starts_with("#["))
        .any(|line| line.trim() == "#[cfg(test)]")
}

impl AuditCorpus {
    fn from_manifest_dir() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = BTreeMap::new();
        for dir in ["src", "tests"] {
            for path in rust_files(&root.join(dir)) {
                let relative = path
                    .strip_prefix(root)
                    .expect("relative source path")
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = fs::read_to_string(&path)
                    .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
                files.insert(relative, text);
            }
        }
        Self { files }
    }

    fn from_pairs<const N: usize>(pairs: [(&str, &str); N]) -> Self {
        Self {
            files: pairs
                .into_iter()
                .map(|(path, text)| (path.to_string(), text.to_string()))
                .collect(),
        }
    }

    fn file(&self, path: &str) -> &str {
        self.files.get(path).map(String::as_str).unwrap_or("")
    }

    fn rust_files(&self) -> impl Iterator<Item = (String, &str)> {
        self.files
            .iter()
            .filter(|(path, _)| {
                path.ends_with(".rs") && path.as_str() != "tests/protection_coverage_audit.rs"
            })
            .map(|(path, text)| (path.clone(), text.as_str()))
    }

    fn src_rust_files(&self) -> impl Iterator<Item = (String, &str)> {
        self.rust_files()
            .filter(|(path, _)| path.starts_with("src/"))
    }
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_rust_files(dir, &mut out);
    out
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|err| panic!("failed to read {dir:?}: {err}")) {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}
