use std::sync::OnceLock;

use toml::value::Table;

use super::{
    CapabilityKind, CapabilitySpec, CatalogError, ParamSpec, ParamType, ProbeCapability,
    ResolvedCapability, required_path, required_path_list, required_u16,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CliCheckKind {
    Probe,
    HelpBinding,
    OutputClaims,
    RerunConsistency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliCapability {
    pub check: CliCheckKind,
    pub entry: String,
    pub usage_paths: Vec<String>,
    pub timeout_seconds: u16,
}

static PARAMS: [ParamSpec; 3] = [
    ParamSpec {
        name: "entry",
        param_type: ParamType::Path,
        required: true,
        default: None,
    },
    ParamSpec {
        name: "usage_paths",
        param_type: ParamType::PathList,
        required: true,
        default: None,
    },
    ParamSpec {
        name: "timeout_seconds",
        param_type: ParamType::U16,
        required: true,
        default: None,
    },
];

const fn probe(id: &'static str, description: &'static str) -> CapabilitySpec {
    CapabilitySpec {
        id,
        kind: CapabilityKind::Probe,
        params: &PARAMS,
        description,
    }
}

static REGISTRY: [CapabilitySpec; 4] = [
    probe("cli_probe", "Run frozen normal and invalid CLI cases."),
    probe(
        "help_binding",
        "Bind runtime help options to parser observations.",
    ),
    probe(
        "cli_output_claims",
        "Bind usage output examples to observed stdout.",
    ),
    probe(
        "cli_rerun_consistency",
        "Compare repeated normal CLI observations.",
    ),
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
    let entry = required_path(spec, params, "entry")?;
    let usage_paths = required_path_list(spec, params, "usage_paths")?;
    let timeout_seconds = required_u16(spec, params, "timeout_seconds")?;
    if !entry.starts_with("cli/") || !entry.ends_with(".py") {
        return Err(invalid(spec, "entry", "expected a .py path under cli/"));
    }
    if timeout_seconds == 0 {
        return Err(invalid(
            spec,
            "timeout_seconds",
            "must be greater than zero",
        ));
    }
    if usage_paths.iter().any(|path| {
        let lower = path.to_ascii_lowercase();
        !lower.contains("readme") && !lower.contains("usage")
    }) {
        return Err(invalid(
            spec,
            "usage_paths",
            "paths must name README or usage documents",
        ));
    }
    let check = match spec.id {
        "cli_probe" => CliCheckKind::Probe,
        "help_binding" => CliCheckKind::HelpBinding,
        "cli_output_claims" => CliCheckKind::OutputClaims,
        "cli_rerun_consistency" => CliCheckKind::RerunConsistency,
        _ => unreachable!("CLI registry id without resolver: {}", spec.id),
    };
    Ok(ResolvedCapability::Probe(ProbeCapability::Cli(
        CliCapability {
            check,
            entry,
            usage_paths,
            timeout_seconds,
        },
    )))
}

fn invalid(spec: &CapabilitySpec, parameter: &str, reason: &str) -> CatalogError {
    CatalogError::InvalidParameter {
        id: spec.id.to_string(),
        parameter: parameter.to_string(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use toml::Value;

    use super::*;

    fn params() -> Table {
        Table::from_iter([
            (
                "entry".to_string(),
                Value::String("cli/main.py".to_string()),
            ),
            (
                "usage_paths".to_string(),
                Value::Array(vec![Value::String("README.md".to_string())]),
            ),
            ("timeout_seconds".to_string(), Value::Integer(5)),
        ])
    }

    #[test]
    fn four_cli_checks_resolve_to_two_component_inputs() {
        for (id, check) in [
            ("cli_probe", CliCheckKind::Probe),
            ("help_binding", CliCheckKind::HelpBinding),
            ("cli_output_claims", CliCheckKind::OutputClaims),
            ("cli_rerun_consistency", CliCheckKind::RerunConsistency),
        ] {
            let ResolvedCapability::Probe(ProbeCapability::Cli(capability)) =
                super::super::resolve(id, &params()).unwrap()
            else {
                panic!("wrong adapter for {id}");
            };
            assert_eq!(capability.check, check);
            assert_eq!(capability.entry, "cli/main.py");
        }
    }

    #[test]
    fn cli_bindings_reject_unsafe_shape_and_zero_timeout() {
        for (key, value) in [
            ("entry", Value::String("pipeline/main.py".to_string())),
            (
                "usage_paths",
                Value::Array(vec![Value::String("notes.txt".to_string())]),
            ),
            ("timeout_seconds", Value::Integer(0)),
        ] {
            let mut invalid_params = params();
            invalid_params.insert(key.to_string(), value);
            assert!(matches!(
                super::super::resolve("cli_probe", &invalid_params),
                Err(CatalogError::InvalidParameter { .. })
            ));
        }
    }
}
