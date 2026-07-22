//! Complete, validated evidence for a workflow-circle adjudication.
//!
//! The writer accepts only a fully described origin binding, every evaluated
//! edge's E-A through E-D details, and the actual node-run identities. This is
//! deliberately stricter than serializing a partially populated JSON value:
//! incomplete evidence aborts before `workflow-circle.json` is created.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OriginReference {
    pub workspace_root: PathBuf,
    pub run_id: String,
    pub events_path: PathBuf,
    pub recovery_yaml_paths: Vec<PathBuf>,
    pub goal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EdgeCheck {
    pub passed: bool,
    pub detail: String,
}

impl EdgeCheck {
    pub fn passed(detail: impl Into<String>) -> Self {
        Self {
            passed: true,
            detail: detail.into(),
        }
    }

    pub fn failed(detail: impl Into<String>) -> Self {
        Self {
            passed: false,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EdgeChecks {
    #[serde(rename = "E-A")]
    pub verdict: EdgeCheck,
    #[serde(rename = "E-B")]
    pub evidence: EdgeCheck,
    #[serde(rename = "E-C")]
    pub epoch: EdgeCheck,
    #[serde(rename = "E-D")]
    pub carry: EdgeCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EdgeRecord {
    pub edge: String,
    pub fired: bool,
    pub checks: EdgeChecks,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRunReference {
    pub intent: String,
    pub run_id: String,
    pub run_dir: PathBuf,
    pub events_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowCircleEvidence {
    pub schema_version: u8,
    pub workflow: String,
    pub origin: OriginReference,
    pub edges: Vec<EdgeRecord>,
    pub nodes: BTreeMap<String, NodeRunReference>,
    pub verdict: Option<String>,
    pub reason: Option<String>,
}

impl WorkflowCircleEvidence {
    pub fn new(workflow: impl Into<String>, origin: OriginReference) -> Self {
        Self {
            schema_version: 1,
            workflow: workflow.into(),
            origin,
            edges: Vec::new(),
            nodes: BTreeMap::new(),
            verdict: None,
            reason: None,
        }
    }

    pub fn record_edge(&mut self, record: EdgeRecord) {
        self.edges.push(record);
    }

    pub fn record_node(
        &mut self,
        node: impl Into<String>,
        reference: NodeRunReference,
    ) -> Result<(), String> {
        let node = node.into();
        if self.nodes.insert(node.clone(), reference).is_some() {
            return Err(format!("duplicate workflow node evidence: {node}"));
        }
        Ok(())
    }

    pub fn adjudicate(&mut self, verdict: &str, reason: Option<&str>) {
        self.verdict = Some(verdict.to_string());
        self.reason = reason.map(str::to_string);
    }

    pub fn write_to(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, bytes).map_err(|e| e.to_string())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || self.workflow.trim().is_empty() {
            return Err("incomplete workflow identity".into());
        }
        if self.origin.workspace_root.as_os_str().is_empty()
            || self.origin.run_id.trim().is_empty()
            || !self.origin.events_path.is_file()
            || self.origin.recovery_yaml_paths.is_empty()
            || self
                .origin
                .recovery_yaml_paths
                .iter()
                .any(|path| !path.is_file())
        {
            return Err("incomplete origin binding reference".into());
        }
        if self
            .origin
            .goal
            .as_ref()
            .is_some_and(|goal| goal.trim().is_empty())
        {
            return Err("incomplete origin goal binding".into());
        }
        let verdict = self
            .verdict
            .as_deref()
            .ok_or_else(|| "missing workflow adjudication".to_string())?;
        if !matches!(
            verdict,
            "circle_full" | "circle_failed" | "circle_interrupted"
        ) {
            return Err("invalid workflow adjudication".into());
        }
        if self.origin.goal.is_none()
            && !(verdict == "circle_failed"
                && self.reason.as_deref() == Some("origin_goal_underivable"))
        {
            return Err("missing origin goal binding".into());
        }
        if self.edges.is_empty() && self.reason.as_deref() != Some("origin_goal_underivable") {
            return Err("missing workflow edge evidence".into());
        }
        for edge in &self.edges {
            if edge.edge.trim().is_empty()
                || edge.checks.verdict.detail.trim().is_empty()
                || edge.checks.evidence.detail.trim().is_empty()
                || edge.checks.epoch.detail.trim().is_empty()
                || edge.checks.carry.detail.trim().is_empty()
            {
                return Err("incomplete workflow edge check details".into());
            }
            let all_passed = edge.checks.verdict.passed
                && edge.checks.evidence.passed
                && edge.checks.epoch.passed
                && edge.checks.carry.passed;
            if edge.fired != all_passed {
                return Err("workflow edge firing disagrees with checks".into());
            }
        }
        for (node, reference) in &self.nodes {
            if node.trim().is_empty()
                || reference.intent.trim().is_empty()
                || reference.run_id == *node
                || uuid::Uuid::parse_str(&reference.run_id).is_err()
                || reference.run_dir.file_name().and_then(|name| name.to_str())
                    != Some(reference.run_id.as_str())
                || reference.events_path.parent() != Some(reference.run_dir.as_path())
                || !reference.run_dir.is_dir()
                || !reference.events_path.is_file()
            {
                return Err("incomplete workflow node/run mapping".into());
            }
        }
        Ok(())
    }
}
