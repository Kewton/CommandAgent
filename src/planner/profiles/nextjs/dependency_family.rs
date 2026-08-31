use serde_json::{Map, Value};

pub(super) type RuntimeFamily = (u64, &'static str);

pub(super) fn ensure_runtime_dependencies(
    dependencies: &mut Map<String, Value>,
) -> Option<RuntimeFamily> {
    if !dependencies.contains_key("next") {
        dependencies.insert("next".to_string(), Value::String("^14.2.0".to_string()));
    }
    let family = match dependencies
        .get("next")
        .and_then(Value::as_str)
        .and_then(semver_major)
    {
        Some(14) => Some((18, "^18.3.0")),
        Some(16) => Some((19, "^19.2.0")),
        _ => None,
    };
    if let Some((major, version)) = family {
        ensure_major(dependencies, "react", version, major);
        ensure_major(dependencies, "react-dom", version, major);
    }
    family
}

pub(super) fn ensure_type_dependencies(
    dependencies: &mut Map<String, Value>,
    family: RuntimeFamily,
) {
    ensure_major(dependencies, "@types/react", family.1, family.0);
    ensure_major(dependencies, "@types/react-dom", family.1, family.0);
}

pub(super) fn coherence_failure(package: &Value, typescript_required: bool) -> Option<String> {
    let Some(next) = version(package, "next") else {
        return Some("next dependency must declare a parseable version".to_string());
    };
    let Some(react) = version(package, "react") else {
        return Some("react dependency must declare a parseable version".to_string());
    };
    let Some(react_dom) = version(package, "react-dom") else {
        return Some("react-dom dependency must declare a parseable version".to_string());
    };
    if typescript_required
        && needs_repair("typescript", version(package, "typescript").unwrap_or(""))
    {
        return Some(
            "typescript dependency must use a deterministic 5.x range such as ^5.5.0".to_string(),
        );
    }
    let Some(next_major) = semver_major(next) else {
        return Some("next dependency must declare a parseable major version".to_string());
    };
    let Some(react_major) = semver_major(react) else {
        return Some("react dependency must declare a parseable major version".to_string());
    };
    let Some(react_dom_major) = semver_major(react_dom) else {
        return Some("react-dom dependency must declare a parseable major version".to_string());
    };
    let expected_react_major = match next_major {
        14 => 18,
        16 => 19,
        unsupported => {
            return Some(format!(
                "Next {unsupported} is unsupported; registered families are Next 14/React 18 and Next 16/React 19"
            ));
        }
    };
    if react_major != expected_react_major || react_dom_major != expected_react_major {
        return Some(format!(
            "Next {next_major} expects React/React DOM {expected_react_major}.x compatibility"
        ));
    }
    for (name, runtime_major) in [
        ("@types/react", react_major),
        ("@types/react-dom", react_dom_major),
    ] {
        if let Some(types) = version(package, name)
            && semver_major(types) != Some(runtime_major)
        {
            return Some(format!("{name} major must match its React runtime major"));
        }
    }
    None
}

pub(super) fn version<'a>(package: &'a Value, name: &str) -> Option<&'a str> {
    ["dependencies", "devDependencies"]
        .iter()
        .filter_map(|key| package.get(*key).and_then(Value::as_object))
        .find_map(|dependencies| dependencies.get(name).and_then(Value::as_str))
}

pub(super) fn needs_repair(name: &str, version: &str) -> bool {
    if version.trim().is_empty() {
        return true;
    }
    match name {
        "typescript" => {
            semver_major(version).is_some_and(|major| major != 5 || version.trim() == "5.0.0")
        }
        "@types/node" => semver_major(version).is_none_or(|major| major != 20),
        "@types/react" | "@types/react-dom" | "react" | "react-dom" => {
            semver_major(version).is_none_or(|major| !matches!(major, 18 | 19))
        }
        "next" => semver_major(version).is_none_or(|major| !matches!(major, 14 | 16)),
        _ => false,
    }
}

fn ensure_major(
    dependencies: &mut Map<String, Value>,
    name: &str,
    version: &str,
    expected_major: u64,
) {
    if dependencies
        .get(name)
        .and_then(Value::as_str)
        .and_then(semver_major)
        != Some(expected_major)
    {
        dependencies.insert(name.to_string(), Value::String(version.to_string()));
    }
}

fn semver_major(version: &str) -> Option<u64> {
    let digits = version
        .trim()
        .trim_start_matches(['^', '~', '=', 'v'])
        .split(|character: char| !character.is_ascii_digit())
        .next()
        .unwrap_or_default();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn next_16_preserves_and_repairs_the_react_19_family() {
        let mut dependencies = json!({
            "next": "16.3.1",
            "react": "18.3.0",
            "react-dom": "18.3.0"
        })
        .as_object()
        .unwrap()
        .clone();

        let family = ensure_runtime_dependencies(&mut dependencies).unwrap();

        assert_eq!(dependencies["next"], "16.3.1");
        assert_eq!(dependencies["react"], "^19.2.0");
        assert_eq!(family.0, 19);
    }

    #[test]
    fn unknown_next_major_is_preserved_and_rejected() {
        let mut package = json!({
            "dependencies": {
                "next": "17.0.0",
                "react": "19.2.8",
                "react-dom": "19.2.8"
            },
            "devDependencies": {"typescript": "^5.5.0"}
        });
        let dependencies = package["dependencies"].as_object_mut().unwrap();

        assert_eq!(ensure_runtime_dependencies(dependencies), None);
        assert_eq!(dependencies["next"], "17.0.0");
        assert!(
            coherence_failure(&package, true)
                .unwrap()
                .contains("Next 17")
        );
    }

    #[test]
    fn plain_javascript_does_not_require_typescript() {
        let package = json!({
            "dependencies": {
                "next": "16.3.1",
                "react": "19.2.8",
                "react-dom": "19.2.8"
            }
        });

        assert_eq!(coherence_failure(&package, false), None);
        assert!(
            coherence_failure(&package, true)
                .unwrap()
                .contains("typescript dependency")
        );
    }
}
