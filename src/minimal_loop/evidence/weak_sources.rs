use super::{VerifyCommandKind, WorkspaceEvidence, verify_command_kind};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WeakEvidenceSource {
    pub source: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
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
            });
        }
    }
    sources
}
