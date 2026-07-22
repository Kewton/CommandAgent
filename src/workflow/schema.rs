//! Strict parser and validator for workflow circle schema v0 and v0.1.
//! YAML is configuration only; execution and adjudication remain in Rust.

use serde::{Deserialize, Deserializer, de};
use std::collections::BTreeMap;
use std::fmt;

use crate::config::Provider;
use crate::planner::profile_admission;
use crate::planner::profile_manifest::ManifestStatus;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    pub workflow: String,
    pub version: WorkflowVersion,
    pub entry: String,
    pub nodes: BTreeMap<String, Node>,
    pub routes: Vec<Route>,
    pub terminal: BTreeMap<String, Terminal>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub intent: Intent,
    pub profile: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider: Option<Provider>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowVersion {
    V0,
    V0_1,
}

impl<'de> Deserialize<'de> for WorkflowVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_yaml::Value::deserialize(deserializer)?;
        match value {
            serde_yaml::Value::Number(number) if number.as_u64() == Some(0) => Ok(Self::V0),
            serde_yaml::Value::Number(number)
                if number
                    .as_f64()
                    .is_some_and(|value| (value - 0.1).abs() < f64::EPSILON) =>
            {
                Ok(Self::V0_1)
            }
            _ => Err(de::Error::custom("workflow version must be 0 or 0.1")),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub from: String,
    pub on: Verdict,
    #[serde(default)]
    pub when: Option<Condition>,
    pub to: String,
    #[serde(default)]
    pub carry: Vec<Carry>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Terminal {
    pub on: Verdict,
    pub verdict: CircleVerdict,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Intent {
    Create,
    Fix,
    Investigate,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Full,
    Failed,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    RecoveryYamlPresent,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Carry {
    Workspace,
    RecoveryYaml,
    ReproducerSuggestion,
    ReproducerLineage,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CircleVerdict {
    CircleFull,
    CircleFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaError(String);

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for SchemaError {}

impl Workflow {
    pub fn parse(input: &str) -> Result<Self, SchemaError> {
        let workflow: Self = serde_yaml::from_str(input)
            .map_err(|e| SchemaError(format!("invalid workflow schema: {e}")))?;
        workflow.validate()?;
        Ok(workflow)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        if !self.nodes.contains_key(&self.entry) {
            return Err(SchemaError("entry node is not defined".into()));
        }
        for (id, node) in &self.nodes {
            if profile_admission::status(&node.profile) != ManifestStatus::Admitted {
                return Err(SchemaError(format!(
                    "node `{id}` uses a non-admitted profile `{}`",
                    node.profile
                )));
            }
            match (&node.model, node.provider) {
                (None, None) => {}
                (Some(model), Some(_)) if model.trim().is_empty() => {
                    return Err(SchemaError(format!("node `{id}` model must not be empty")));
                }
                (Some(_), Some(_)) if self.version == WorkflowVersion::V0 => {
                    return Err(SchemaError(format!(
                        "node `{id}` executor override requires workflow version 0.1"
                    )));
                }
                (Some(_), Some(_)) => {}
                (Some(_), None) => {
                    return Err(SchemaError(format!("node `{id}` model requires provider")));
                }
                (None, Some(_)) => {
                    return Err(SchemaError(format!("node `{id}` provider requires model")));
                }
            }
        }
        for route in &self.routes {
            if !self.nodes.contains_key(&route.from) || !self.nodes.contains_key(&route.to) {
                return Err(SchemaError("route references an unknown node".into()));
            }
            if self.version == WorkflowVersion::V0
                && route.carry.contains(&Carry::ReproducerSuggestion)
            {
                return Err(SchemaError(
                    "reproducer_suggestion carry requires workflow version 0.1".into(),
                ));
            }
        }
        for (id, terminal) in &self.terminal {
            if !self.nodes.contains_key(id) {
                return Err(SchemaError("terminal references an unknown node".into()));
            }
            if terminal.verdict == CircleVerdict::CircleFull && terminal.on != Verdict::Full {
                return Err(SchemaError("circle_full terminal must be on full".into()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const VALID: &str = "workflow: recovery-circle-data\nversion: 0\nentry: create\nnodes:\n  create: {intent: create, profile: generic}\n  investigate: {intent: investigate, profile: generic}\nroutes:\n  - {from: create, on: failed, when: recovery_yaml_present, to: investigate, carry: [workspace, recovery_yaml]}\nterminal: {}\n";
    const VALID_V0_1: &str = "workflow: recovery-circle-data\nversion: 0.1\nentry: create\nnodes:\n  create: {intent: create, profile: generic}\n  investigate: {intent: investigate, profile: generic, model: elevated-model, provider: gemini}\nroutes:\n  - {from: create, on: failed, when: recovery_yaml_present, to: investigate, carry: [workspace, recovery_yaml]}\nterminal: {}\n";

    #[test]
    fn parses_valid_schema() {
        assert!(Workflow::parse(VALID).is_ok());
        let workflow = Workflow::parse(VALID_V0_1).unwrap();
        let investigate = &workflow.nodes["investigate"];
        assert_eq!(investigate.model.as_deref(), Some("elevated-model"));
        assert_eq!(investigate.provider, Some(Provider::Gemini));
    }

    #[test]
    fn rejects_unknown_key() {
        assert!(Workflow::parse(&VALID.replace("version: 0", "version: 0\nextra: true")).is_err());
        assert!(
            Workflow::parse(&VALID_V0_1.replace(
                "model: elevated-model",
                "planner_model: elevated-planner, model: elevated-model"
            ))
            .is_err()
        );
    }

    #[test]
    fn executor_override_requires_v0_1_model_provider_pair() {
        assert!(
            Workflow::parse(&VALID_V0_1.replace(", provider: gemini", ""))
                .unwrap_err()
                .to_string()
                .contains("model requires provider")
        );
        assert!(
            Workflow::parse(&VALID_V0_1.replace(", model: elevated-model", ""))
                .unwrap_err()
                .to_string()
                .contains("provider requires model")
        );
        assert!(
            Workflow::parse(&VALID_V0_1.replace("version: 0.1", "version: 0"))
                .unwrap_err()
                .to_string()
                .contains("requires workflow version 0.1")
        );
        assert!(Workflow::parse(&VALID_V0_1.replace("gemini", "unknown-provider")).is_err());
    }

    #[test]
    fn reproducer_suggestion_is_a_strict_v0_1_carry_keyword() {
        let v0_1 = VALID_V0_1.replace(
            "carry: [workspace, recovery_yaml]",
            "carry: [workspace, recovery_yaml, reproducer_suggestion]",
        );
        assert!(Workflow::parse(&v0_1).is_ok());
        assert!(
            Workflow::parse(&v0_1.replace("version: 0.1", "version: 0"))
                .unwrap_err()
                .to_string()
                .contains("requires workflow version 0.1")
        );
        assert!(
            Workflow::parse(&v0_1.replace("reproducer_suggestion", "reproducer_hint")).is_err()
        );
    }

    #[test]
    fn elevated_template_parses_after_measurement_substitution() {
        let template = include_str!("../../workflows/recovery-circle-data-elevated.yaml");
        let resolved = template
            .replace("<investigate-executor-id>", "investigate-model")
            .replace("<investigate-provider-id>", "ollama")
            .replace("<fix-executor-id>", "fix-model")
            .replace("<fix-provider-id>", "gemini");
        assert!(Workflow::parse(&resolved).is_ok());
    }
}
