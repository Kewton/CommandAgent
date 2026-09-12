//! Fail-closed recognition of Node JSON writers. This is deliberately a small
//! static language: builtin imports, immutable lexical bindings, literal paths,
//! process.cwd(), and path.join(). Unsupported expressions grant no output.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use crate::minimal_loop::completion::CompletionContract;

mod forwarding;
mod generic_renames;
mod renames;
mod syntax;
use syntax::{Source, Token};

pub(super) fn registered_paths(
    workspace: &Path,
    contract: &CompletionContract,
) -> BTreeSet<String> {
    // The observer starts at the workspace root. A nested project or a command
    // selecting another cwd needs a separate product-bound execution root;
    // never infer process.cwd() from the source file's directory.
    if !workspace.join("package.json").is_file()
        || !package_scripts_preserve_cwd(workspace)
        || !(workspace.join("src/app").is_dir() || workspace.join("app").is_dir())
        || contract
            .verify_commands
            .iter()
            .chain(contract.fix_reproducer_command.iter())
            .any(|command| changes_command_cwd(command))
    {
        return BTreeSet::new();
    }
    let candidates = crate::minimal_loop::import_scan::nextjs_route_bound_closure(workspace);
    // A route-bound module can change the process cwd before another module's
    // writer runs. This exclusion may match comments too; it only removes grants.
    if candidates.iter().any(|path| {
        confined_path(workspace, path)
            && std::fs::read_to_string(workspace.join(path))
                .is_ok_and(|text| text.contains("chdir"))
    }) {
        return BTreeSet::new();
    }
    let mut pending = crate::planner::profiles::nextjs::app_source_paths(workspace)
        .into_iter()
        .map(PathBuf::from)
        .filter(|path| {
            candidates.contains(path)
                && matches!(
                    path.file_stem().and_then(|s| s.to_str()),
                    Some("route" | "page" | "layout")
                )
        })
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    let mut outputs = BTreeSet::new();
    while let Some(path) = pending.pop() {
        if !visited.insert(path.clone()) || !confined_path(workspace, &path) {
            continue;
        }
        let Some(source) = std::fs::read_to_string(workspace.join(&path))
            .ok()
            .and_then(|text| Source::parse(&text))
        else {
            continue;
        };
        // Recheck edges as tokens: a quoted/commented import must not register
        // a sibling merely because the broader diagnostic import scan found it.
        for specifier in source.imports() {
            let bases = if specifier.starts_with('.') {
                vec![path.parent().unwrap_or(Path::new("")).join(specifier)]
            } else if let Some(tail) = specifier.strip_prefix("@/") {
                let prefix = path
                    .components()
                    .take_while(|c| c.as_os_str() != "src" && c.as_os_str() != "app")
                    .collect::<PathBuf>();
                vec![prefix.join("src").join(tail), prefix.join(tail)]
            } else {
                Vec::new()
            };
            for base in bases {
                let Some(base) = normalize_import(&base) else {
                    continue;
                };
                for suffix in [
                    "",
                    ".ts",
                    ".tsx",
                    ".js",
                    ".jsx",
                    "/index.ts",
                    "/index.tsx",
                    "/index.js",
                ] {
                    let resolved = PathBuf::from(format!("{}{suffix}", base.display()));
                    if candidates.contains(&resolved) {
                        pending.push(resolved);
                        break;
                    }
                }
            }
        }
        // Exported generic helpers are only supported when all their references
        // are local. Token-like words also reject opaque/unsupported importers.
        let foreign_names = generic_renames::foreign_references(workspace, &path, &candidates)
            .unwrap_or_else(|| {
                (0..source.tokens.len())
                    .filter_map(|i| source.identifier(i).map(str::to_owned))
                    .collect()
            });
        for output in source.writer_paths_in_closure(&foreign_names) {
            let Some(output) = normalized_literal(&output) else {
                continue;
            };
            let path = Path::new(&output);
            if path.extension().is_some_and(|ext| ext == "json")
                && !path.components().any(|part| {
                    let name = part.as_os_str().to_string_lossy();
                    name.starts_with('.')
                        || matches!(
                            name.as_ref(),
                            "src"
                                | "app"
                                | "pages"
                                | "lib"
                                | "components"
                                | "scripts"
                                | "config"
                                | "node_modules"
                                | "credentials.json"
                        )
                })
                && confined_path(workspace, path)
                && !workspace.join(path).is_dir()
            {
                outputs.insert(output);
            }
        }
    }
    outputs
}

fn changes_command_cwd(command: &str) -> bool {
    command.contains("chdir")
        || command
            .split(|ch: char| ch.is_whitespace() || ";&|()\"'".contains(ch))
            .any(|word| {
                matches!(word, "cd" | "pushd" | "-w")
                    || word.starts_with("-C")
                    || ["--prefix", "--cwd", "--dir", "--workspace"]
                        .iter()
                        .any(|flag| word.starts_with(flag))
            })
}

fn package_scripts_preserve_cwd(workspace: &Path) -> bool {
    let Some(package) = std::fs::read(workspace.join("package.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
    else {
        return false;
    };
    // Check all scripts, including lifecycle hooks and indirectly invoked npm
    // scripts. An unrelated cwd-changing script is a conservative false negative.
    package.get("scripts").is_none_or(|scripts| {
        scripts.as_object().is_some_and(|scripts| {
            scripts.values().all(|script| {
                script
                    .as_str()
                    .is_some_and(|body| !changes_command_cwd(body))
            })
        })
    })
}

fn normalize_import(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir if normalized.pop() => {}
            _ => return None,
        }
    }
    Some(normalized)
}

fn normalized_literal(value: &str) -> Option<String> {
    if value.is_empty() || value.contains(['\\', ':', '\0']) {
        return None;
    }
    let mut parts = Vec::new();
    for part in Path::new(value).components() {
        match part {
            Component::Normal(part) => parts.push(part.to_str()?),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Check every existing component, including dangling links and the leaf.
/// A missing leaf (or directory) is expected for lazy initialization.
fn confined_path(root: &Path, relative: &Path) -> bool {
    let mut current = root.to_path_buf();
    for part in relative.components() {
        let Component::Normal(part) = part else {
            return false;
        };
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return false,
            Ok(metadata) if current != root.join(relative) && !metadata.is_dir() => return false,
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return false,
        }
    }
    true
}

#[derive(Clone, Debug)]
enum Value {
    Path(String),
    Fs,
    Promises,
    Writer,
    Renamer,
    PathModule,
    Join,
}

#[derive(Clone)]
struct Binding {
    scope: usize,
    position: usize,
    value: Option<Value>,
    expression: Option<usize>,
}

impl Source {
    #[cfg(test)]
    fn writer_paths(&self) -> Vec<String> {
        self.writer_paths_in_closure(&BTreeSet::new())
    }

    fn writer_paths_in_closure(&self, foreign_names: &BTreeSet<String>) -> Vec<String> {
        let mut bindings: BTreeMap<String, Vec<Binding>> = BTreeMap::new();
        for (name, value, position) in self.builtin_bindings() {
            bindings.entry(name).or_default().push(Binding {
                scope: 0,
                position,
                value: Some(value),
                expression: None,
            });
        }
        for i in 0..self.tokens.len() {
            if self.is(i, "const")
                && let Some(name) = self.identifier(i + 1)
            {
                bindings.entry(name.to_string()).or_default().push(Binding {
                    scope: self.scopes[i],
                    position: i,
                    value: None,
                    expression: self.is(i + 2, "=").then_some(i + 3),
                });
            }
        }
        let mut paths = Vec::new();
        for i in 0..self.tokens.len() {
            if i > 0 && self.is(i - 1, ".") {
                continue;
            }
            if let Some((Value::Writer, end)) = self.expression(i, &bindings, 0)
                && self.is(end, "(")
                && let Some((Value::Path(path), end)) = self.expression(end + 1, &bindings, 0)
                && self.is(end, ",")
            {
                paths.push(path);
            }
        }
        paths.extend(self.forwarded_writer_paths(&bindings));
        paths.extend(self.renamed_paths(&bindings));
        paths.extend(self.generic_rename_paths(&bindings, foreign_names));
        paths
    }

    fn expression(
        &self,
        start: usize,
        bindings: &BTreeMap<String, Vec<Binding>>,
        depth: u8,
    ) -> Option<(Value, usize)> {
        if depth > 16 {
            return None;
        }
        let (mut value, mut end) = match self.tokens.get(start)? {
            Token::String(value) => (Value::Path(normalized_literal(value)?), start + 1),
            Token::Word(name)
                if name == "process"
                    && !self.unsafe_names.contains(name)
                    && !bindings.contains_key(name)
                    && self.sequence(start + 1, &[".", "cwd", "(", ")"]) =>
            {
                (Value::Path(String::new()), start + 5)
            }
            Token::Word(name) if !self.unsafe_names.contains(name) => {
                let definitions = bindings.get(name)?;
                let mut scope = self.scopes[start];
                let definition = loop {
                    let local = definitions
                        .iter()
                        .filter(|binding| binding.scope == scope)
                        .collect::<Vec<_>>();
                    if !local.is_empty() {
                        if local.len() != 1 || local[0].position >= start {
                            return None;
                        }
                        break local[0];
                    }
                    scope = *self.parents.get(scope)?.as_ref()?;
                };
                let value = if let Some(value) = &definition.value {
                    value.clone()
                } else {
                    let (value, end) =
                        self.expression(definition.expression?, bindings, depth + 1)?;
                    // Do not accept a prefix of a dynamic initializer.
                    if !self.is(end, ";") {
                        return None;
                    }
                    value
                };
                (value, start + 1)
            }
            _ => return None,
        };
        while self.is(end, ".") {
            value = match (&value, self.identifier(end + 1)?) {
                (Value::Fs, "promises") => Value::Promises,
                (Value::Fs, "writeFile" | "writeFileSync") | (Value::Promises, "writeFile") => {
                    Value::Writer
                }
                (Value::Fs, "rename" | "renameSync") | (Value::Promises, "rename") => {
                    Value::Renamer
                }
                (Value::PathModule, "join") => Value::Join,
                _ => return None,
            };
            end += 2;
        }
        if matches!(value, Value::Join) && self.is(end, "(") {
            let mut parts = Vec::new();
            end += 1;
            loop {
                let (Value::Path(part), next) = self.expression(end, bindings, depth + 1)? else {
                    return None;
                };
                if !part.is_empty() {
                    parts.push(part);
                }
                end = next;
                if self.is(end, ")") {
                    break;
                }
                if !self.is(end, ",") {
                    return None;
                }
                end += 1;
            }
            value = Value::Path(normalized_literal(&parts.join("/"))?);
            end += 1;
        }
        Some((value, end))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod reopen_tests;

#[cfg(test)]
mod issue467_tests;

#[cfg(test)]
mod issue475_tests;
