use std::collections::HashSet;
use std::env::VarError;
use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};

pub const CURRENT_PREFIX: &str = "COMMANDAGENT_";
const LEGACY_PREFIX: &str = "ANVIL_";

static WARNED_LEGACY_NAMES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

pub fn var(name: &str) -> Result<String, VarError> {
    match var_os(name) {
        Some(value) => value.into_string().map_err(VarError::NotUnicode),
        None => Err(VarError::NotPresent),
    }
}

pub fn var_os(name: &str) -> Option<OsString> {
    var_with(name, |key| std::env::var_os(key))
}

pub fn var_with<T>(name: &str, get_env: impl Fn(&str) -> Option<T>) -> Option<T> {
    resolve_with(
        name,
        get_env,
        warned_legacy_names(),
        |legacy_name, current_name| {
            eprintln!("warning: {legacy_name} is deprecated; use {current_name} instead");
        },
    )
}

pub fn legacy_name(name: &str) -> Option<String> {
    name.strip_prefix(CURRENT_PREFIX)
        .map(|suffix| format!("{LEGACY_PREFIX}{suffix}"))
}

pub fn matches_current_or_legacy(actual: &str, current: &str) -> bool {
    actual == current || legacy_name(current).is_some_and(|legacy| actual == legacy)
}

fn warned_legacy_names() -> &'static Mutex<HashSet<String>> {
    WARNED_LEGACY_NAMES.get_or_init(|| Mutex::new(HashSet::new()))
}

fn resolve_with<T>(
    current_name: &str,
    get_env: impl Fn(&str) -> Option<T>,
    warned_names: &Mutex<HashSet<String>>,
    mut warn: impl FnMut(&str, &str),
) -> Option<T> {
    if let Some(value) = get_env(current_name) {
        return Some(value);
    }
    let legacy_name = legacy_name(current_name)?;
    let value = get_env(&legacy_name)?;
    let should_warn = warned_names
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(legacy_name.clone());
    if should_warn {
        warn(&legacy_name, current_name);
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const CURRENT: &str = "COMMANDAGENT_TEST_VALUE";
    const LEGACY: &str = "ANVIL_TEST_VALUE";

    fn resolve_case(values: &[(&str, &str)]) -> (Option<String>, Vec<String>) {
        let values = values.iter().copied().collect::<HashMap<_, _>>();
        let warnings = Mutex::new(HashSet::new());
        let mut emitted = Vec::new();
        let value = resolve_with(
            CURRENT,
            |name| values.get(name).map(|value| (*value).to_string()),
            &warnings,
            |legacy, current| emitted.push(format!("{legacy}->{current}")),
        );
        (value, emitted)
    }

    #[test]
    fn current_and_legacy_environment_precedence_matrix() {
        let (value, warnings) = resolve_case(&[(CURRENT, "current")]);
        assert_eq!(value.as_deref(), Some("current"));
        assert!(warnings.is_empty());

        let (value, warnings) = resolve_case(&[(LEGACY, "legacy")]);
        assert_eq!(value.as_deref(), Some("legacy"));
        assert_eq!(warnings, [format!("{LEGACY}->{CURRENT}")]);

        let (value, warnings) = resolve_case(&[(CURRENT, "current"), (LEGACY, "legacy")]);
        assert_eq!(value.as_deref(), Some("current"));
        assert!(warnings.is_empty());

        let (value, warnings) = resolve_case(&[]);
        assert_eq!(value, None);
        assert!(warnings.is_empty());
    }

    #[test]
    fn legacy_only_environment_warns_once() {
        let warnings = Mutex::new(HashSet::new());
        let mut emitted = Vec::new();
        for _ in 0..2 {
            let value = resolve_with(
                CURRENT,
                |name| (name == LEGACY).then(|| "legacy".to_string()),
                &warnings,
                |legacy, current| emitted.push(format!("{legacy}->{current}")),
            );
            assert_eq!(value.as_deref(), Some("legacy"));
        }
        assert_eq!(emitted, [format!("{LEGACY}->{CURRENT}")]);
    }

    #[test]
    fn non_commandagent_names_do_not_gain_a_legacy_fallback() {
        let warnings = Mutex::new(HashSet::new());
        let value = resolve_with(
            "NO_COLOR",
            |name| (name == "NO_COLOR").then(|| "1".to_string()),
            &warnings,
            |_, _| panic!("unexpected warning"),
        );
        assert_eq!(value.as_deref(), Some("1"));
        assert_eq!(legacy_name("NO_COLOR"), None);
    }
}
