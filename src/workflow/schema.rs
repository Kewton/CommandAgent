//! Strict parser and validator for workflow circle schema v0.
//! YAML is configuration only; execution and adjudication remain in Rust.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;

use crate::planner::profile_admission;
use crate::planner::profile_manifest::ManifestStatus;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    pub workflow: String,
    pub version: u8,
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
pub enum Intent { Create, Fix, Investigate }

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Verdict { Full, Failed }

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Condition { RecoveryYamlPresent }

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Carry { Workspace, RecoveryYaml, ReproducerLineage }

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CircleVerdict { CircleFull, CircleFailed }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaError(String);

impl fmt::Display for SchemaError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) } }
impl std::error::Error for SchemaError {}

impl Workflow {
    pub fn parse(input: &str) -> Result<Self, SchemaError> {
        let workflow: Self = serde_yaml::from_str(input).map_err(|e| SchemaError(format!("invalid workflow schema: {e}")))?;
        workflow.validate()?;
        Ok(workflow)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        if self.version != 0 { return Err(SchemaError("workflow version must be 0".into())); }
        if !self.nodes.contains_key(&self.entry) { return Err(SchemaError("entry node is not defined".into())); }
        for (id, node) in &self.nodes {
            if profile_admission::status(&node.profile) != ManifestStatus::Admitted {
                return Err(SchemaError(format!("node `{id}` uses a non-admitted profile `{}`", node.profile)));
            }
        }
        for route in &self.routes {
            if !self.nodes.contains_key(&route.from) || !self.nodes.contains_key(&route.to) { return Err(SchemaError("route references an unknown node".into())); }
        }
        for (id, terminal) in &self.terminal {
            if !self.nodes.contains_key(id) { return Err(SchemaError("terminal references an unknown node".into())); }
            if terminal.verdict == CircleVerdict::CircleFull && terminal.on != Verdict::Full { return Err(SchemaError("circle_full terminal must be on full".into())); }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const VALID: &str = "workflow: recovery-circle-data\nversion: 0\nentry: create\nnodes:\n  create: {intent: create, profile: generic}\n  investigate: {intent: investigate, profile: generic}\nroutes:\n  - {from: create, on: failed, when: recovery_yaml_present, to: investigate, carry: [workspace, recovery_yaml]}\nterminal: {}\n";
    #[test] fn parses_valid_schema() { assert!(Workflow::parse(VALID).is_ok()); }
    #[test] fn rejects_unknown_key() { assert!(Workflow::parse(&VALID.replace("version: 0", "version: 0\nextra: true")).is_err()); }
}
