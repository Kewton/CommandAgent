use std::cell::RefCell;
use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::path::Path;
use std::rc::Rc;

use clap::ValueEnum;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum AllowTarget {
    #[value(name = "read")]
    Read,
    #[value(name = "write")]
    Write,
    #[value(name = "bash:verify")]
    BashVerify,
}

impl AllowTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::BashVerify => "bash:verify",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum ActivePolicy {
    #[default]
    LegacyApproval,
    Explicit(BTreeSet<AllowTarget>),
    All,
}

thread_local! {
    static ACTIVE_POLICY: RefCell<ActivePolicy> = const { RefCell::new(ActivePolicy::LegacyApproval) };
}

pub(crate) struct ScopedPolicy {
    previous: ActivePolicy,
    _not_send: PhantomData<Rc<()>>,
}

impl Drop for ScopedPolicy {
    fn drop(&mut self) {
        ACTIVE_POLICY.with(|active| {
            *active.borrow_mut() = std::mem::take(&mut self.previous);
        });
    }
}

pub(crate) fn install(yes: bool, allowed: &[AllowTarget]) -> ScopedPolicy {
    let next = if yes {
        ActivePolicy::All
    } else if allowed.is_empty() {
        ActivePolicy::LegacyApproval
    } else {
        ActivePolicy::Explicit(allowed.iter().copied().collect())
    };
    let previous = ACTIVE_POLICY.with(|active| std::mem::replace(&mut *active.borrow_mut(), next));
    ScopedPolicy {
        previous,
        _not_send: PhantomData,
    }
}

pub(crate) fn ensure_current_allows(
    tool: &str,
    arguments: &Value,
    workspace_root: &Path,
) -> anyhow::Result<()> {
    ACTIVE_POLICY.with(|active| {
        active
            .borrow()
            .ensure_allows(tool, arguments, workspace_root)
    })
}

pub(crate) fn authorize_current(
    tool: &str,
    arguments: &Value,
    workspace_root: &Path,
    legacy_auto_approve: bool,
    interactive_approval: bool,
) -> anyhow::Result<()> {
    ensure_current_allows(tool, arguments, workspace_root)?;
    if matches!(tool, "Write" | "Edit" | "Bash") {
        let policy_auto_approves = ACTIVE_POLICY.with(|active| {
            active
                .borrow()
                .auto_approves(tool, arguments, workspace_root)
        });
        super::approval::require_tool_approval(
            tool,
            legacy_auto_approve || policy_auto_approves,
            interactive_approval,
        )?;
    }
    Ok(())
}

pub(crate) fn current_has_mutating_authority() -> bool {
    ACTIVE_POLICY.with(|active| match &*active.borrow() {
        ActivePolicy::LegacyApproval => false,
        ActivePolicy::Explicit(allowed) => {
            allowed.contains(&AllowTarget::Write) || allowed.contains(&AllowTarget::BashVerify)
        }
        ActivePolicy::All => true,
    })
}

impl ActivePolicy {
    fn ensure_allows(
        &self,
        tool: &str,
        arguments: &Value,
        workspace_root: &Path,
    ) -> anyhow::Result<()> {
        let Self::Explicit(allowed) = self else {
            return Ok(());
        };
        let permitted = match tool {
            "Read" | "Glob" | "Grep" => allowed.contains(&AllowTarget::Read),
            "Write" | "Edit" => allowed.contains(&AllowTarget::Write),
            "Bash" if allowed.contains(&AllowTarget::BashVerify) => {
                let command = arguments
                    .get("command")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("Bash requires string argument `command`"))?;
                crate::planner::verify::normalize_runtime_bash_command_for_boundary(
                    command,
                    workspace_root,
                )
                .map(|plan| {
                    plan.segments.iter().all(|segment| {
                        matches!(
                            &segment.command,
                            crate::planner::verify::RuntimeNormalizedCommand::Verify(_)
                        )
                    }) && !super::bash_write_guard::has_recognized_mutation(
                        &plan.normalized_command,
                    )
                })
                .unwrap_or(false)
            }
            _ => false,
        };
        if permitted {
            return Ok(());
        }
        let active = allowed
            .iter()
            .map(|target| target.as_str())
            .collect::<Vec<_>>()
            .join(",");
        anyhow::bail!(
            "tool {tool} is not permitted by --allow {active}; do not broaden permissions; use Write for filesystem mutation because it creates parent directories automatically, and keep Bash for allowed build, test, or read-only verification commands"
        )
    }

    fn auto_approves(&self, tool: &str, arguments: &Value, workspace_root: &Path) -> bool {
        match self {
            Self::LegacyApproval => false,
            Self::All => true,
            Self::Explicit(_) => self.ensure_allows(tool, arguments, workspace_root).is_ok(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn explicit_policy_is_a_hard_tool_ceiling() {
        let root = tempfile::tempdir().unwrap();
        let _guard = install(false, &[AllowTarget::Read, AllowTarget::Write]);

        assert!(ensure_current_allows("Read", &json!({"path":"a"}), root.path()).is_ok());
        assert!(ensure_current_allows("Write", &json!({"path":"a"}), root.path()).is_ok());
        assert!(
            ensure_current_allows("Bash", &json!({"command":"cargo test"}), root.path()).is_err()
        );
    }

    #[test]
    fn bash_verify_uses_verify_policy_and_rejects_direct_mutation() {
        let root = tempfile::tempdir().unwrap();
        let _guard = install(false, &[AllowTarget::BashVerify]);

        assert!(
            ensure_current_allows("Bash", &json!({"command":"cargo test"}), root.path()).is_ok()
        );
        assert!(
            authorize_current(
                "Bash",
                &json!({"command":"cargo test"}),
                root.path(),
                false,
                false,
            )
            .is_ok()
        );
        assert!(
            ensure_current_allows("Bash", &json!({"command":"npm install"}), root.path()).is_err()
        );
        assert!(
            ensure_current_allows("Bash", &json!({"command":"touch marker"}), root.path()).is_err()
        );
    }

    #[test]
    fn scoped_policy_restores_legacy_behavior() {
        let root = tempfile::tempdir().unwrap();
        {
            let _guard = install(false, &[AllowTarget::Read]);
            assert!(ensure_current_allows("Write", &json!({"path":"a"}), root.path()).is_err());
        }
        assert!(ensure_current_allows("Write", &json!({"path":"a"}), root.path()).is_ok());
        assert!(
            authorize_current(
                "Bash",
                &json!({"command":"cargo test"}),
                root.path(),
                false,
                false,
            )
            .is_err()
        );
    }
}
