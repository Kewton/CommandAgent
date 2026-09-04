use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ClientMutation {
    method: String,
    route: String,
    source: String,
    response_checked: bool,
}

pub(super) fn failure(root: &Path) -> Option<String> {
    let mutations = client_mutations(root);
    if mutations.is_empty() {
        return None;
    }
    let routes = api_routes(root);
    for mutation in mutations {
        let Some((path, content)) = routes
            .iter()
            .find(|(path, _)| route_matches(path, &mutation.route))
        else {
            return Some(format!(
                "api_contract_failure: {} {} used by {} has no matching App Router route",
                mutation.method, mutation.route, mutation.source
            ));
        };
        if !exports_method(content, &mutation.method) {
            return Some(format!(
                "api_contract_failure: {} {} used by {} is not exported by {}",
                mutation.method,
                mutation.route,
                mutation.source,
                path.display()
            ));
        }
        if !mutation.response_checked {
            return Some(format!(
                "api_contract_failure: {} {} used by {} does not check Response.ok before accepting the mutation",
                mutation.method, mutation.route, mutation.source
            ));
        }
    }
    None
}

fn client_mutations(root: &Path) -> BTreeSet<ClientMutation> {
    let pattern = Regex::new(
        r#"(?is)fetch\s*\(\s*["'`]([^"'`]+)["'`]\s*,\s*\{[^)]{0,1200}?\bmethod\s*:\s*["'`](POST|PUT|PATCH|DELETE)["'`]"#,
    )
    .expect("valid Next.js fetch mutation regex");
    let mut out = BTreeSet::new();
    for path in source_files(root) {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for captures in pattern.captures_iter(&content) {
            let Some(route) = captures
                .get(1)
                .map(|value| normalize_client_route(value.as_str()))
            else {
                continue;
            };
            if !route.starts_with("/api/") {
                continue;
            }
            let method = captures
                .get(2)
                .map(|value| value.as_str().to_ascii_uppercase())
                .unwrap_or_default();
            out.insert(ClientMutation {
                method,
                route,
                source: path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/"),
                response_checked: mutation_response_checked(&content, captures.get(0)),
            });
        }
    }
    out
}

fn mutation_response_checked(content: &str, matched: Option<regex::Match<'_>>) -> bool {
    let Some(matched) = matched else {
        return false;
    };
    let prefix_start = content[..matched.start()]
        .rfind([';', '\n', '{'])
        .map_or(0, |index| index + 1);
    let prefix = content[prefix_start..matched.start()].trim();
    let suffix_end = matched.end().saturating_add(4_096).min(content.len());
    let suffix = &content[matched.end()..suffix_end];
    if prefix.starts_with("return ") || suffix.contains(").ok") || suffix.contains(").ok;") {
        return true;
    }
    let assignment =
        Regex::new(r"(?s)(?:const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*=\s*(?:await\s*)?$")
            .expect("valid fetch response assignment regex");
    let Some(name) = assignment
        .captures(prefix)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str())
    else {
        return false;
    };
    Regex::new(&format!(r"\b{}\s*\.\s*ok\b", regex::escape(name)))
        .expect("valid response ok regex")
        .is_match(suffix)
}

fn api_routes(root: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    for path in source_files(root) {
        if api_route_segments(root, &path).is_none() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        out.push((relative, content));
    }
    out.sort_by(|left, right| left.0.cmp(&right.0));
    out
}

fn route_matches(route_path: &Path, client_route: &str) -> bool {
    let Some(route_segments) = api_route_segments(Path::new(""), route_path) else {
        return false;
    };
    let client_segments = client_route
        .trim_matches('/')
        .split('/')
        .skip_while(|segment| *segment != "api")
        .skip(1)
        .collect::<Vec<_>>();
    route_segments.len() == client_segments.len()
        && route_segments
            .iter()
            .zip(client_segments)
            .all(|(registered, observed)| {
                (registered.starts_with('[') && registered.ends_with(']')) || registered == observed
            })
}

fn api_route_segments(root: &Path, path: &Path) -> Option<Vec<String>> {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let parts = relative
        .components()
        .filter_map(|part| part.as_os_str().to_str())
        .collect::<Vec<_>>();
    let app_index = parts.iter().position(|part| *part == "app")?;
    if parts.get(app_index + 1) != Some(&"api") {
        return None;
    }
    let file = parts.last()?.to_ascii_lowercase();
    if !matches!(
        file.as_str(),
        "route.ts" | "route.tsx" | "route.js" | "route.jsx"
    ) {
        return None;
    }
    Some(
        parts[app_index + 2..parts.len() - 1]
            .iter()
            .map(|part| (*part).to_string())
            .collect(),
    )
}

fn exports_method(content: &str, method: &str) -> bool {
    let pattern = Regex::new(&format!(
        r"(?m)\bexport\s+(?:(?:async\s+)?function|const)\s+{}\b",
        regex::escape(method)
    ))
    .expect("valid route method export regex");
    pattern.is_match(content)
}

fn normalize_client_route(route: &str) -> String {
    route
        .split(['?', '#'])
        .next()
        .unwrap_or(route)
        .trim_end_matches('/')
        .to_string()
}

fn source_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(
                name.as_ref(),
                ".git" | ".next" | ".commandagent" | ".anvil" | "node_modules"
            ) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| {
                        matches!(
                            value.to_ascii_lowercase().as_str(),
                            "ts" | "tsx" | "js" | "jsx"
                        )
                    })
            {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, path: &str, content: &str) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn rejects_client_method_missing_from_route_exports() {
        let root = tempfile::tempdir().unwrap();
        write(
            root.path(),
            "src/app/page.tsx",
            r#"fetch("/api/todos", { method: "DELETE" });"#,
        );
        write(
            root.path(),
            "src/app/api/todos/route.ts",
            "export async function GET() {}\nexport async function POST() {}\n",
        );

        let reason = failure(root.path()).expect("missing DELETE must fail");

        assert!(reason.contains("DELETE /api/todos"), "{reason}");
        assert!(reason.contains("src/app/api/todos/route.ts"), "{reason}");
    }

    #[test]
    fn accepts_exported_methods_and_dynamic_routes() {
        let root = tempfile::tempdir().unwrap();
        write(
            root.path(),
            "src/app/page.tsx",
            r#"
const patchResponse = await fetch(`/api/tasks/${id}`, { method: "PATCH" });
if (!patchResponse.ok) throw new Error("patch failed");
const postResponse = await fetch("/api/tasks", { method: "POST" });
if (!postResponse.ok) throw new Error("post failed");
const deleteResponse = await fetch(`/api/tasks/${id}`, { method: "DELETE" });
if (!deleteResponse.ok) throw new Error("delete failed");
const clearResponse = await fetch("/api/tasks", { method: "DELETE" });
if (!clearResponse.ok) throw new Error("clear failed");
"#,
        );
        write(
            root.path(),
            "src/app/api/tasks/route.ts",
            "export async function POST() {}\nexport async function DELETE() {}\n",
        );
        write(
            root.path(),
            "src/app/api/tasks/[id]/route.ts",
            "export const PATCH = async () => {};\nexport async function DELETE() {}\n",
        );

        assert_eq!(failure(root.path()), None);
    }

    #[test]
    fn rejects_ignored_mutation_response() {
        let root = tempfile::tempdir().unwrap();
        write(
            root.path(),
            "src/app/page.tsx",
            r#"await fetch("/api/tasks", { method: "DELETE" });"#,
        );
        write(
            root.path(),
            "src/app/api/tasks/route.ts",
            "export async function DELETE() {}\n",
        );

        let reason = failure(root.path()).expect("unchecked response must fail");

        assert!(reason.contains("does not check Response.ok"), "{reason}");
    }

    #[test]
    fn does_not_borrow_method_from_a_later_fetch_call() {
        let root = tempfile::tempdir().unwrap();
        write(
            root.path(),
            "src/app/page.tsx",
            r#"
fetch("/api/read-only", { cache: "no-store" });
const response = await fetch("/api/tasks", { method: "POST" });
if (!response.ok) throw new Error("post failed");
"#,
        );
        write(
            root.path(),
            "src/app/api/tasks/route.ts",
            "export async function POST() {}\n",
        );

        assert_eq!(failure(root.path()), None);
    }
}
