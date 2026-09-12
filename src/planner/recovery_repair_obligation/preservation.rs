//! Conservative preservation checks complement (never replace) registered checks.
use super::*;
use regex::Regex;

pub(super) fn is_source(path: &str) -> bool {
    matches!(
        Path::new(path).extension().and_then(|s| s.to_str()),
        Some("ts" | "tsx" | "js" | "jsx" | "mjs" | "py")
    )
}

pub(super) fn freeze_verifiers(
    config: &Config,
    contract: &CompletionContract,
    sources: &BTreeMap<String, String>,
) -> anyhow::Result<BTreeMap<String, String>> {
    let mut frozen = configuration::snapshot(config)?;
    let root = &config.workspace_root;
    for entry in ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_global(false)
        .filter_entry(|e| {
            !matches!(
                e.file_name().to_str(),
                Some(".git" | ".anvil" | ".commandagent" | "node_modules" | "target" | ".next")
            )
        })
        .build()
    {
        let entry = entry?;
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        if sources.contains_key(&relative) {
            continue;
        }
        let verifier = contract
            .verify_commands
            .iter()
            .any(|c| c.contains(&relative))
            || ["checks", "tests", "scripts"]
                .iter()
                .any(|dir| Path::new(&relative).starts_with(dir))
            || contract
                .protected_paths
                .iter()
                .any(|p| Path::new(&relative).starts_with(p));
        if verifier {
            frozen.insert(
                relative,
                format!("{:x}", Sha256::digest(std::fs::read(entry.path())?)),
            );
        }
    }
    Ok(frozen)
}

pub(super) fn verify(config: &Config, obligation: &Obligation) -> anyhow::Result<()> {
    configuration::verify(config, obligation)?;
    for (path, before) in &obligation.frozen {
        let bytes = std::fs::read(resolve_existing(&config.workspace_root, path)?)?;
        ensure!(
            format!("{:x}", Sha256::digest(bytes)) == *before,
            "Recovery confirmation input changed: {path}; preserve registered checks and compiler configuration"
        );
    }
    let referenced: Vec<_> = obligation
        .sources
        .values()
        .flat_map(|text| aliases(text).into_values())
        .collect();
    for (path, before) in &obligation.sources {
        let after = read_source(&config.workspace_root, path)
            .with_context(|| format!("Recovery target source removed: {path}"))?;
        if before == &after {
            continue;
        }
        let before_surface = surface(before);
        let after_surface = surface(&after);
        for (symbol, _) in before_surface {
            let (kind, name) = symbol.split_once(':').unwrap();
            let required = if kind == "export" {
                obligation
                    .targets
                    .iter()
                    .any(|target| Path::new(target) == Path::new(path))
                    || referenced.iter().any(|reference| reference == name)
            } else {
                Regex::new(&format!(r"\b{}\b", regex::escape(name)))
                    .unwrap()
                    .is_match(&obligation.diagnostics)
                    || aliases(before).values().any(|reference| reference == name)
            };
            if !required {
                continue;
            }
            ensure!(
                after_surface.contains_key(&symbol),
                "Recovery source processing removed in {path}: {symbol}; preserve the diagnosed call and API surface"
            );
        }
        for suppression in [
            "@ts-ignore",
            "@ts-nocheck",
            "@ts-expect-error",
            "type: ignore",
        ] {
            ensure!(
                after.matches(suppression).count() <= before.matches(suppression).count(),
                "Recovery diagnostic suppression added in {path}: {suppression}"
            );
        }
        ensure!(
            bypasses(&after) <= bypasses(before),
            "Recovery type-check bypass added in {path}"
        );
    }
    Ok(())
}

fn bypasses(text: &str) -> usize {
    // Import aliases and const assertions do not suppress type checking.
    let source = code(text);
    let imports = Regex::new(r"(?s)\b(?:import|export)\s*\{[^}]*\}").unwrap();
    let source = imports.replace_all(&source, "");
    let casts = Regex::new(r"\bas\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap();
    let any = Regex::new(r"\bany\b|\bcast\s*\(").unwrap();
    any.find_iter(&source).count()
        + casts
            .captures_iter(&source)
            .filter(|c| &c[1] != "const")
            .count()
}

fn aliases(text: &str) -> BTreeMap<String, String> {
    let imports = Regex::new(r"(?s)\bimport\s*\{([^}]*)\}").unwrap();
    let mut result = BTreeMap::new();
    for capture in imports.captures_iter(&code(text)) {
        for part in capture[1].split(',') {
            let words: Vec<_> = part.split_whitespace().filter(|w| *w != "type").collect();
            match words.as_slice() {
                [name] => {
                    result.insert((*name).to_string(), (*name).to_string());
                }
                [name, "as", alias] => {
                    result.insert((*alias).to_string(), (*name).to_string());
                }
                _ => {}
            }
        }
    }
    result
}

// Strip comments and quoted contents before measuring executable call/export
// surfaces. A comment or string containing the old call cannot preserve it.
fn code(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') {
            for c in chars.by_ref() {
                if c == '\n' {
                    break;
                }
            }
            result.push('\n');
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(c) = chars.next() {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
            result.push(' ');
        } else if matches!(c, '\'' | '"' | '`') {
            while let Some(next) = chars.next() {
                if next == '\\' {
                    chars.next();
                } else if next == c {
                    break;
                }
            }
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}

fn surface(text: &str) -> BTreeMap<String, usize> {
    let aliases = aliases(text);
    let code = code(text);
    let calls = Regex::new(r"\b([A-Za-z_$][A-Za-z0-9_$]*)\s*\(").unwrap();
    let exports = Regex::new(r"\bexport\s+(?:async\s+)?(?:function|const|class|interface|type)\s+([A-Za-z_$][A-Za-z0-9_$]*)").unwrap();
    let mut result = BTreeMap::new();
    for captures in calls.captures_iter(&code) {
        if matches!(
            &captures[1],
            "if" | "for" | "while" | "switch" | "catch" | "function"
        ) {
            continue;
        }
        let name = aliases
            .get(&captures[1])
            .map(String::as_str)
            .unwrap_or(&captures[1]);
        *result.entry(format!("call:{name}")).or_default() += 1;
    }
    for captures in exports.captures_iter(&code) {
        *result
            .entry(format!("export:{}", &captures[1]))
            .or_default() += 1;
    }
    result
}
