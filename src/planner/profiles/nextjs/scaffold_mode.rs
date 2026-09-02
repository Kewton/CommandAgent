use std::path::Path;

use serde_json::Value;

use super::knowledge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScaffoldMode {
    CanonicalTypeScriptTailwind,
    ExistingTypeScriptPlainCss,
    ExistingJavaScriptTailwind,
    ExistingPlainJavaScript,
}

pub(super) fn detect(root: &Path) -> ScaffoldMode {
    let Some(package) = read_package(root) else {
        return ScaffoldMode::CanonicalTypeScriptTailwind;
    };
    match (
        uses_typescript_toolchain(root, &package),
        uses_tailwind_toolchain(root, &package),
    ) {
        (true, true) => ScaffoldMode::CanonicalTypeScriptTailwind,
        (true, false) => ScaffoldMode::ExistingTypeScriptPlainCss,
        (false, true) => ScaffoldMode::ExistingJavaScriptTailwind,
        (false, false) => ScaffoldMode::ExistingPlainJavaScript,
    }
}

impl ScaffoldMode {
    pub(super) fn uses_typescript(self) -> bool {
        matches!(
            self,
            Self::CanonicalTypeScriptTailwind | Self::ExistingTypeScriptPlainCss
        )
    }

    pub(super) fn uses_tailwind(self) -> bool {
        matches!(
            self,
            Self::CanonicalTypeScriptTailwind | Self::ExistingJavaScriptTailwind
        )
    }
}

pub(super) fn required_paths(root: &Path) -> Vec<String> {
    let paths = match detect(root) {
        ScaffoldMode::CanonicalTypeScriptTailwind => knowledge::get()
            .canonical
            .scaffold_files
            .iter()
            .map(|rel| rel.replace("{tailwind_config}", tailwind_config_rel(root)))
            .collect(),
        ScaffoldMode::ExistingTypeScriptPlainCss => [
            "package.json",
            "tsconfig.json",
            "src/app/layout.tsx",
            "src/app/page.tsx",
            "src/app/globals.css",
            "src/app/global.d.ts",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
        ScaffoldMode::ExistingJavaScriptTailwind => vec![
            "package.json".to_string(),
            "postcss.config.js".to_string(),
            tailwind_config_rel(root).to_string(),
            "src/app/layout.js".to_string(),
            "src/app/page.js".to_string(),
            "src/app/globals.css".to_string(),
        ],
        ScaffoldMode::ExistingPlainJavaScript => [
            "package.json",
            "src/app/layout.js",
            "src/app/page.js",
            "src/app/globals.css",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
    };
    paths
        .into_iter()
        .map(|path| canonicalize_existing_app_path(root, &path))
        .filter(|path| !optional_absent_globals_css(root, path))
        .collect()
}

pub(super) fn canonicalize_existing_app_path(root: &Path, value: &str) -> String {
    if root.join("app").is_dir() && !root.join("src/app").exists() {
        value
            .replace("src/app/", "app/")
            .replace("src\\app\\", "app\\")
    } else {
        value.to_string()
    }
}

pub(super) fn optional_absent_globals_css(root: &Path, path: &str) -> bool {
    if !path.replace('\\', "/").ends_with("app/globals.css") || root.join(path).exists() {
        return false;
    }
    let layout = [
        "src/app/layout.tsx",
        "src/app/layout.jsx",
        "src/app/layout.ts",
        "src/app/layout.js",
        "app/layout.tsx",
        "app/layout.jsx",
        "app/layout.ts",
        "app/layout.js",
    ]
    .into_iter()
    .find(|layout| root.join(layout).is_file());
    layout.is_some_and(|layout| {
        std::fs::read_to_string(root.join(layout))
            .is_ok_and(|content| !content.contains("globals.css"))
    })
}

pub(super) fn tailwind_config_rel(root: &Path) -> &'static str {
    if let Some(existing) = knowledge::get()
        .canonical
        .tailwind_config_rels
        .iter()
        .map(String::as_str)
        .find(|rel| root.join(rel).is_file())
    {
        return existing;
    }
    if detect(root) == ScaffoldMode::ExistingJavaScriptTailwind {
        return "tailwind.config.js";
    }
    knowledge::get()
        .canonical
        .tailwind_config_rels
        .first()
        .map(String::as_str)
        .expect("embedded Next.js tailwind config candidates must not be empty")
}

pub(super) fn uses_typescript_toolchain(root: &Path, package: &Value) -> bool {
    [
        "typescript",
        "@types/node",
        "@types/react",
        "@types/react-dom",
    ]
    .iter()
    .any(|name| package_has_dependency(package, name))
        || root.join("tsconfig.json").is_file()
        || source_tree_contains_extension(root, &["ts", "tsx"])
}

fn uses_tailwind_toolchain(root: &Path, package: &Value) -> bool {
    package_has_dependency(package, "tailwindcss")
        || [
            "tailwind.config.ts",
            "tailwind.config.js",
            "tailwind.config.cjs",
            "tailwind.config.mjs",
            "postcss.config.js",
            "postcss.config.cjs",
            "postcss.config.mjs",
        ]
        .iter()
        .any(|rel| root.join(rel).is_file())
        || source_tree_contains(root, "@tailwind")
}

fn read_package(root: &Path) -> Option<Value> {
    let content = std::fs::read_to_string(root.join("package.json")).ok()?;
    serde_json::from_str(&content).ok()
}

fn package_has_dependency(package: &Value, name: &str) -> bool {
    ["dependencies", "devDependencies"]
        .iter()
        .filter_map(|section| package.get(section).and_then(Value::as_object))
        .any(|dependencies| dependencies.contains_key(name))
}

fn source_tree_contains_extension(root: &Path, extensions: &[&str]) -> bool {
    source_files(root).any(|path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extensions.contains(&extension))
    })
}

fn source_tree_contains(root: &Path, needle: &str) -> bool {
    source_files(root)
        .any(|path| std::fs::read_to_string(path).is_ok_and(|content| content.contains(needle)))
}

fn source_files(root: &Path) -> impl Iterator<Item = std::path::PathBuf> {
    ["src/app", "app", "src/pages", "pages"]
        .into_iter()
        .flat_map(|rel| collect_files(&root.join(rel)))
}

fn collect_files(root: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .flat_map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                collect_files(&path)
            } else {
                vec![path]
            }
        })
        .collect()
}

pub(super) fn plain_layout() -> &'static str {
    r#"import "./globals.css";

export const metadata = {
  title: "Interactive Challenge",
  description: "A compact interactive challenge generated by commandagent",
};

export default function RootLayout({ children }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
"#
}

pub(super) fn plain_page() -> &'static str {
    r#""use client";

import { useState } from "react";

export default function Page() {
  const [count, setCount] = useState(0);
  return (
    <main data-anvil-state={JSON.stringify({ count })}>
      <h1>INTERACTIVE CHALLENGE</h1>
      <p>Count: {count}</p>
      <button data-anvil-action="primary" onClick={() => setCount((value) => value + 1)}>
        Start
      </button>
      <button data-anvil-action="restart" onClick={() => setCount(0)}>
        Reset
      </button>
    </main>
  );
}
"#
}

pub(super) fn plain_css() -> &'static str {
    "* { box-sizing: border-box; }\nhtml, body { margin: 0; min-height: 100%; }\nbody { background: #05070d; color: #eef7ff; font-family: sans-serif; }\nmain { min-height: 100vh; padding: 2rem; }\nbutton { margin-right: 0.75rem; font: inherit; }\n"
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::Value;

    use super::super::{
        auto_repair, complete_scaffold, package_has_dependency, repair_manifest_coherence,
        setup_scaffold_paths, verify,
    };

    fn package_script(root: &Path, name: &str) -> Option<String> {
        let package: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("package.json")).ok()?).ok()?;
        package
            .get("scripts")?
            .get(name)?
            .as_str()
            .map(str::to_string)
    }

    #[test]
    fn existing_minimal_manifest_uses_plain_javascript_scaffold() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name":"plain-app","private":true}"#,
        )
        .unwrap();

        let paths = setup_scaffold_paths(dir.path());
        assert_eq!(
            paths,
            vec![
                "package.json",
                "src/app/layout.js",
                "src/app/page.js",
                "src/app/globals.css",
            ]
        );

        let created = complete_scaffold(dir.path(), &paths).unwrap();
        assert_eq!(
            created,
            vec![
                "src/app/layout.js",
                "src/app/page.js",
                "src/app/globals.css",
            ]
        );
        assert!(!dir.path().join("tsconfig.json").exists());
        assert!(!dir.path().join("postcss.config.js").exists());
        assert!(!dir.path().join("tailwind.config.ts").exists());
        let page = std::fs::read_to_string(dir.path().join("src/app/page.js")).unwrap();
        assert!(page.contains("data-anvil-state"), "{page}");
        assert!(page.contains("data-anvil-action=\"primary\""), "{page}");
    }

    #[test]
    fn existing_root_app_router_tree_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app")).unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name":"root-app","private":true}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("app/layout.js"),
            "export default function Layout({children}){return children;}\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("app/page.js"),
            "export default function Page(){return null;}\n",
        )
        .unwrap();

        let paths = setup_scaffold_paths(dir.path());

        assert!(paths.contains(&"app/layout.js".to_string()), "{paths:?}");
        assert!(paths.contains(&"app/page.js".to_string()), "{paths:?}");
        assert!(!paths.contains(&"app/globals.css".to_string()), "{paths:?}");
        assert!(!paths.iter().any(|path| path.starts_with("src/app/")));
        assert!(complete_scaffold(dir.path(), &paths).unwrap().is_empty());
        assert!(!dir.path().join("src/app").exists());

        let report = crate::planner::profile::profile_failure("dev script missing");
        assert!(auto_repair(dir.path(), "port 4185", &report).unwrap());
        assert!(!dir.path().join("src/app").exists());
        assert!(!dir.path().join("tsconfig.json").exists());
        let package: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("package.json")).unwrap(),
        )
        .unwrap();
        assert!(!package_has_dependency(&package, "typescript"));
        assert!(verify(dir.path(), "port 4185").is_pass());
    }

    #[test]
    fn imported_missing_globals_css_remains_required() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app")).unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name":"root-app","private":true}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("app/layout.js"),
            "import './globals.css';\nexport default function Layout({children}){return children;}\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("app/page.js"),
            "export default function Page(){return null;}\n",
        )
        .unwrap();

        let paths = setup_scaffold_paths(dir.path());

        assert!(paths.contains(&"app/globals.css".to_string()), "{paths:?}");
    }

    #[test]
    fn manifest_repair_preserves_plain_javascript_dependency_boundary() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name":"plain-app","private":true}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("src/app/page.js"),
            "export default function Page(){return <main>ok</main>;}\n",
        )
        .unwrap();

        assert!(repair_manifest_coherence(dir.path(), "port 4201").unwrap());

        let package: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("package.json")).unwrap(),
        )
        .unwrap();
        assert!(
            package
                .get("devDependencies")
                .and_then(Value::as_object)
                .is_none()
        );
        for forbidden in [
            "typescript",
            "@types/node",
            "@types/react",
            "@types/react-dom",
            "tailwindcss",
            "postcss",
            "autoprefixer",
        ] {
            assert!(!package_has_dependency(&package, forbidden), "{forbidden}");
        }
        assert_eq!(
            package_script(dir.path(), "dev").as_deref(),
            Some("next dev -p 4201")
        );
        assert_eq!(
            package_script(dir.path(), "build").as_deref(),
            Some("next build")
        );
    }

    #[test]
    fn javascript_tailwind_scaffold_does_not_add_typescript() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name":"js-tailwind","private":true,"devDependencies":{"tailwindcss":"^3.4.0","postcss":"^8.0.0","autoprefixer":"^10.0.0"}}"#,
        )
        .unwrap();

        let paths = setup_scaffold_paths(dir.path());
        assert!(paths.contains(&"src/app/page.js".to_string()), "{paths:?}");
        assert!(
            paths.contains(&"tailwind.config.js".to_string()),
            "{paths:?}"
        );
        assert!(
            !paths.iter().any(|path| path.ends_with(".tsx")),
            "{paths:?}"
        );
        assert!(!paths.contains(&"tsconfig.json".to_string()), "{paths:?}");

        assert!(repair_manifest_coherence(dir.path(), "port 4201").unwrap());
        let package: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("package.json")).unwrap(),
        )
        .unwrap();
        for forbidden in [
            "typescript",
            "@types/node",
            "@types/react",
            "@types/react-dom",
        ] {
            assert!(!package_has_dependency(&package, forbidden), "{forbidden}");
        }
    }

    #[test]
    fn typescript_plain_css_scaffold_does_not_add_tailwind() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name":"ts-plain","private":true,"devDependencies":{"typescript":"^5.5.0","@types/node":"^20.0.0","@types/react":"^18.0.0","@types/react-dom":"^18.0.0"}}"#,
        )
        .unwrap();

        let paths = setup_scaffold_paths(dir.path());
        assert!(paths.contains(&"src/app/page.tsx".to_string()), "{paths:?}");
        assert!(paths.contains(&"tsconfig.json".to_string()), "{paths:?}");
        assert!(
            !paths.contains(&"postcss.config.js".to_string()),
            "{paths:?}"
        );
        assert!(!paths.iter().any(|path| path.starts_with("tailwind.config")));

        assert!(repair_manifest_coherence(dir.path(), "port 4201").unwrap());
        let package: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("package.json")).unwrap(),
        )
        .unwrap();
        for forbidden in ["tailwindcss", "postcss", "autoprefixer"] {
            assert!(!package_has_dependency(&package, forbidden), "{forbidden}");
        }
    }
}
