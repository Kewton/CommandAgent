use std::sync::OnceLock;

use toml::Value;
use toml::value::Table;

use super::{
    CapabilityKind, CapabilitySpec, CatalogError, InternalCapability, ParamSpec, ParamType,
    ResolvedCapability,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackInternalCheck {
    PathLayoutConforms {
        required: Vec<String>,
        forbidden: Vec<String>,
    },
    DesignTokensOnly {
        css_globs: Vec<String>,
        tokens_file: String,
        allow: Vec<String>,
    },
    LintConfigPresent {
        path: String,
        must_contain: Vec<String>,
    },
}

static PATH_LAYOUT_PARAMS: [ParamSpec; 2] = [
    ParamSpec {
        name: "required",
        param_type: ParamType::GlobList,
        required: true,
        default: None,
    },
    ParamSpec {
        name: "forbidden",
        param_type: ParamType::GlobList,
        required: false,
        default: Some("[]"),
    },
];
static DESIGN_TOKENS_PARAMS: [ParamSpec; 3] = [
    ParamSpec {
        name: "css_globs",
        param_type: ParamType::GlobList,
        required: true,
        default: None,
    },
    ParamSpec {
        name: "tokens_file",
        param_type: ParamType::Path,
        required: true,
        default: None,
    },
    ParamSpec {
        name: "allow",
        param_type: ParamType::StringList,
        required: false,
        default: Some("[]"),
    },
];
static LINT_CONFIG_PARAMS: [ParamSpec; 2] = [
    ParamSpec {
        name: "path",
        param_type: ParamType::Path,
        required: true,
        default: None,
    },
    ParamSpec {
        name: "must_contain",
        param_type: ParamType::StringList,
        required: false,
        default: Some("[]"),
    },
];

static REGISTRY: [CapabilitySpec; 3] = [
    CapabilitySpec {
        id: "path_layout_conforms",
        kind: CapabilityKind::InternalCheck,
        params: &PATH_LAYOUT_PARAMS,
        description: "Require and forbid bounded workspace-relative glob matches.",
    },
    CapabilitySpec {
        id: "design_tokens_only",
        kind: CapabilityKind::InternalCheck,
        params: &DESIGN_TOKENS_PARAMS,
        description: "Reject raw CSS color literals outside the selected token file.",
    },
    CapabilitySpec {
        id: "lint_config_present",
        kind: CapabilityKind::InternalCheck,
        params: &LINT_CONFIG_PARAMS,
        description: "Require a lint configuration file and selected literals.",
    },
];

pub(super) fn combined_registry(base: &'static [CapabilitySpec]) -> &'static [CapabilitySpec] {
    static COMBINED: OnceLock<Vec<CapabilitySpec>> = OnceLock::new();
    COMBINED.get_or_init(|| base.iter().chain(REGISTRY.iter()).copied().collect())
}

pub(super) fn is_id(id: &str) -> bool {
    REGISTRY.iter().any(|spec| spec.id == id)
}

pub(super) fn resolve(
    spec: &CapabilitySpec,
    params: &Table,
) -> Result<ResolvedCapability, CatalogError> {
    let check = match spec.id {
        "path_layout_conforms" => PackInternalCheck::PathLayoutConforms {
            required: required_glob_list(spec, params, "required")?,
            forbidden: optional_glob_list(spec, params, "forbidden")?,
        },
        "design_tokens_only" => PackInternalCheck::DesignTokensOnly {
            css_globs: required_glob_list(spec, params, "css_globs")?,
            tokens_file: super::required_path(spec, params, "tokens_file")?,
            allow: optional_string_list(spec, params, "allow")?,
        },
        "lint_config_present" => PackInternalCheck::LintConfigPresent {
            path: super::required_path(spec, params, "path")?,
            must_contain: optional_string_list(spec, params, "must_contain")?,
        },
        _ => unreachable!("pack registry id without resolver: {}", spec.id),
    };
    Ok(ResolvedCapability::Internal(InternalCapability::Pack(
        check,
    )))
}

fn required_glob_list(
    spec: &CapabilitySpec,
    params: &Table,
    name: &str,
) -> Result<Vec<String>, CatalogError> {
    string_list(spec, params, name, ParamType::GlobList, false, |value| {
        crate::tools::path_guard::validate_workspace_relative(value)
            .map_err(|error| error.to_string())?;
        if value.contains('\\') {
            return Err("glob must use POSIX separators".to_string());
        }
        globset::Glob::new(value)
            .map(|_| ())
            .map_err(|error| format!("invalid glob: {error}"))
    })
}

fn optional_glob_list(
    spec: &CapabilitySpec,
    params: &Table,
    name: &str,
) -> Result<Vec<String>, CatalogError> {
    if !params.contains_key(name) {
        return Ok(Vec::new());
    }
    string_list(spec, params, name, ParamType::GlobList, true, |value| {
        crate::tools::path_guard::validate_workspace_relative(value)
            .map_err(|error| error.to_string())?;
        if value.contains('\\') {
            return Err("glob must use POSIX separators".to_string());
        }
        globset::Glob::new(value)
            .map(|_| ())
            .map_err(|error| format!("invalid glob: {error}"))
    })
}

fn optional_string_list(
    spec: &CapabilitySpec,
    params: &Table,
    name: &str,
) -> Result<Vec<String>, CatalogError> {
    if !params.contains_key(name) {
        return Ok(Vec::new());
    }
    string_list(spec, params, name, ParamType::StringList, true, |value| {
        if value.is_empty() {
            Err("literal must not be empty".to_string())
        } else {
            Ok(())
        }
    })
}

fn string_list(
    spec: &CapabilitySpec,
    params: &Table,
    name: &str,
    expected: ParamType,
    allow_empty: bool,
    validate: impl Fn(&str) -> Result<(), String>,
) -> Result<Vec<String>, CatalogError> {
    let Value::Array(values) = super::required_value(spec, params, name)? else {
        return Err(super::type_mismatch(spec, name, expected));
    };
    if !allow_empty && values.is_empty() {
        return Err(invalid(spec, name, "list may not be empty"));
    }
    if values.len() > 64 {
        return Err(invalid(spec, name, "list may contain at most 64 entries"));
    }
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        let Value::String(value) = value else {
            return Err(super::type_mismatch(spec, name, expected));
        };
        validate(value).map_err(|reason| invalid(spec, name, &reason))?;
        if out.contains(value) {
            return Err(invalid(spec, name, &format!("duplicate entry `{value}`")));
        }
        out.push(value.clone());
    }
    Ok(out)
}

fn invalid(spec: &CapabilitySpec, name: &str, reason: &str) -> CatalogError {
    CatalogError::InvalidParameter {
        id: spec.id.to_string(),
        parameter: name.to_string(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn additive_checks_enforce_closed_glob_and_literal_bounds() {
        let layout_spec = spec("path_layout_conforms");
        let mut layout = Table::new();
        layout.insert(
            "required".to_string(),
            Value::Array(vec![Value::String("src/app/**".to_string())]),
        );
        assert!(matches!(
            resolve(layout_spec, &layout).unwrap(),
            ResolvedCapability::Internal(InternalCapability::Pack(
                PackInternalCheck::PathLayoutConforms { .. }
            ))
        ));

        layout.insert(
            "required".to_string(),
            Value::Array(vec![Value::String("../outside/**".to_string())]),
        );
        assert!(matches!(
            resolve(layout_spec, &layout),
            Err(CatalogError::InvalidParameter { parameter, .. }) if parameter == "required"
        ));

        layout.insert(
            "required".to_string(),
            Value::Array(
                (0..65)
                    .map(|index| Value::String(format!("src/{index}")))
                    .collect(),
            ),
        );
        assert!(matches!(
            resolve(layout_spec, &layout),
            Err(CatalogError::InvalidParameter { parameter, .. }) if parameter == "required"
        ));

        let mut lint = Table::new();
        lint.insert(
            "path".to_string(),
            Value::String("eslint.config.mjs".to_string()),
        );
        lint.insert(
            "must_contain".to_string(),
            Value::Array(vec![Value::String(String::new())]),
        );
        assert!(matches!(
            resolve(spec("lint_config_present"), &lint),
            Err(CatalogError::InvalidParameter { parameter, .. }) if parameter == "must_contain"
        ));
    }

    fn spec(id: &str) -> &'static CapabilitySpec {
        REGISTRY.iter().find(|spec| spec.id == id).unwrap()
    }
}
