//! Configuration absence is part of the host confirmation conditions too.
use super::*;

pub(super) fn is_configuration(path: &str) -> bool {
    let name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    name.starts_with("tsconfig")
        || name.starts_with("jsconfig")
        || name.contains(".config.")
        || [".babelrc", ".swcrc", ".eslintrc", ".mocharc", ".env"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        || matches!(
            name.as_str(),
            "package.json"
                | ".npmrc"
                | ".yarnrc"
                | ".yarnrc.yml"
                | "biome.json"
                | "biome.jsonc"
                | "deno.json"
                | "deno.jsonc"
                | "turbo.json"
                | "pyproject.toml"
                | "mypy.ini"
                | ".mypy.ini"
                | "pytest.ini"
                | "tox.ini"
                | "setup.cfg"
                | "cargo.toml"
        )
}

pub(super) fn snapshot(config: &Config) -> anyhow::Result<BTreeMap<String, String>> {
    let root = &config.workspace_root;
    let mut inputs = BTreeMap::new();
    // Ignore rules control versioning, not whether the compiler reads a config.
    for entry in ignore::WalkBuilder::new(root)
        .hidden(false)
        .ignore(false)
        .git_ignore(false)
        .git_exclude(false)
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
        if entry.file_type().is_some_and(|kind| kind.is_dir()) {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        if is_configuration(&relative) {
            // Includes symlinked configs; an outside-workspace redirect fails.
            let path = resolve_existing(root, &relative)?;
            inputs.insert(
                relative,
                format!("{:x}", Sha256::digest(std::fs::read(path)?)),
            );
        }
    }
    Ok(inputs)
}

pub(super) fn verify(config: &Config, obligation: &Obligation) -> anyhow::Result<()> {
    for (path, hash) in snapshot(config)? {
        let before = obligation.frozen.get(&path)
            .with_context(|| format!("Recovery confirmation configuration added: {path}; preserve original compiler and verification conditions"))?;
        ensure!(
            hash == *before,
            "Recovery confirmation configuration changed: {path}"
        );
    }
    // The frozen-input check also rejects deletions and unreadable originals.
    Ok(())
}
