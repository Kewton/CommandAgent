use super::verify_command_classification::{node_repair_guidance, node_weak_detail};
use super::{VerifyCommandKind, WorkspaceEvidence, verify_command_kind};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WeakEvidenceSource {
    pub source: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// First reason a Node verify command was refused as assertion evidence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl WeakEvidenceSource {
    /// A repair line naming the command, the first refusal and the accepted
    /// shape. Only sources that carry a detail have one.
    pub fn repair_guidance(&self) -> Option<String> {
        Some(node_repair_guidance(
            self.command.as_deref()?,
            self.detail.as_deref()?,
        ))
    }
}

pub(super) fn collect(
    reasons: &[String],
    commands: &[String],
    workspace: &WorkspaceEvidence,
) -> Vec<WeakEvidenceSource> {
    let mut sources = Vec::new();
    for reason in reasons {
        let before = sources.len();
        for command in commands {
            let command_reason = match verify_command_kind(command, workspace) {
                VerifyCommandKind::Weak(reason) => reason,
                VerifyCommandKind::ArtifactOnly => format!("artifact_only_verify:{command}"),
                _ => continue,
            };
            if command_reason == *reason {
                let entry = WeakEvidenceSource {
                    source: "contract_verify_command".into(),
                    reason: reason.clone(),
                    command: Some(command.clone()),
                    detail: node_weak_detail(command, workspace),
                };
                if !sources.contains(&entry) {
                    sources.push(entry);
                }
            }
        }
        if sources.len() == before {
            sources.push(WeakEvidenceSource {
                source: if reason.starts_with("route_unbound:") {
                    "route_unbound"
                } else {
                    "obligation"
                }
                .into(),
                reason: reason.clone(),
                command: None,
                detail: None,
            });
        }
    }
    sources
}
