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

/// Whether an explicit `bash:verify` policy auto-approves this command.
///
/// A command whose text contains a comment, heredoc, or line continuation is
/// never auto-approved. The lexical write guard elides that text before it
/// inspects the command, while the verify classification inspects the command
/// with newlines replaced, so the two would otherwise read different commands
/// and a comment could swallow the next line (Issue #576).
pub fn bash_verify_command_is_auto_approvable(command: &str, workspace_root: &Path) -> bool {
    if !super::shell_lexical::text_is_unchanged(command) {
        return false;
    }
    // The verify normalization replaces vertical tabs and Unicode spaces with a
    // normal space, so it can turn a `#x;rm -rf src` tail into a comment. The raw
    // text must not be a recognized mutation on its own (B6).
    if super::bash_write_guard::verifier_mutation::rejection(command, false).is_some() {
        return false;
    }
    crate::planner::verify::normalize_runtime_bash_command_for_boundary(command, workspace_root)
        .map(|plan| {
            plan.segments.iter().all(|segment| {
                matches!(
                    &segment.command,
                    crate::planner::verify::RuntimeNormalizedCommand::Verify(_)
                )
            }) && super::bash_write_guard::verifier_mutation::rejection(
                &plan.normalized_command,
                false,
            )
            .is_none()
        })
        .unwrap_or(false)
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
                bash_verify_command_is_auto_approvable(command, workspace_root)
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

    #[test]
    fn issue861_bash_verify_rejects_in_place_edits_and_keeps_read_only_forms() {
        let root = tempfile::tempdir().unwrap();
        let _guard = install(false, &[AllowTarget::BashVerify]);
        for command in [
            "sed -i 's/1/2/' src/a.ts",
            "sed -i '' 's/1/2/' src/a.ts",
            "sed --in-place s/1/2/ src/a.ts",
            "perl -pi -e 's/1/2/' src/a.ts",
            "perl -i.bak -pe 's/1/2/' src/a.ts",
            "env FOO=1 /usr/bin/sed -i s/1/2/ src/a.ts",
            "npm run build && sed -i s/1/2/ src/a.ts",
            "cp always-pass.sh smoke-check.sh",
            "sed -l -i '' s/1/2/ src/a.ts",
            "timeout -sKILL 5 sed -i s/1/2/ src/a.ts",
            "nice -n5 perl -pi -e x src/a.ts",
        ] {
            let arguments = json!({ "command": command });
            assert!(
                ensure_current_allows("Bash", &arguments, root.path()).is_err(),
                "{command}"
            );
            assert!(
                authorize_current("Bash", &arguments, root.path(), false, false).is_err(),
                "{command}"
            );
        }
        for command in [
            "sed -n '1,5p' src/a.ts",
            "npm run build",
            "node smoke-check.js",
        ] {
            let arguments = json!({ "command": command });
            assert!(
                ensure_current_allows("Bash", &arguments, root.path()).is_ok(),
                "{command}"
            );
        }
    }

    #[test]
    fn issue861_bash_verify_classifier_reads_the_normalized_text_too() {
        let root = tempfile::tempdir().unwrap();
        // A vertical tab is a blank to the verify normalization but not to the
        // lexical guard, so the update only shows in the normalized reading.
        assert!(!bash_verify_command_is_auto_approvable(
            "npm run build\u{b}&&\u{b}sed -i s/1/2/ src/a.ts",
            root.path()
        ));
    }

    #[test]
    fn issue871_bash_verify_rejects_opaque_forms_whatever_their_body() {
        let root = tempfile::tempdir().unwrap();
        let _guard = install(false, &[AllowTarget::BashVerify]);
        for command in [
            "sh -c 'cat a.txt'",
            "sh -c 'cp always-pass.sh smoke-check.sh'",
            "bash -lc 'npm test'",
            "/bin/sh -c true",
            "env FOO=1 timeout 5 sh -c true",
            "timeout -sKILL 5 sh -c true",
            "eval true",
            "xargs cat",
            "busybox cat a.txt",
            "find . -exec cat {} +",
            "find . -delete",
            "node -e 'console.log(1)'",
            "node -p 1",
            "node --eval=1",
            "python3 -c 'print(1)'",
            "perl -e 'print 1'",
            "perl -ne 'print' a.txt",
            "sed 's/a/b/' a.txt",
            "sed -n '1,5w out.txt' a.txt",
            "gsed -f prog.sed a.txt",
            "sh make-sentinel.sh && sh -c true",
            "sh\u{b}-c\u{b}true",
        ] {
            let arguments = json!({ "command": command });
            assert!(
                ensure_current_allows("Bash", &arguments, root.path()).is_err(),
                "{command}"
            );
            assert!(
                authorize_current("Bash", &arguments, root.path(), false, false).is_err(),
                "{command}"
            );
        }
    }

    #[test]
    fn issue871_bash_verify_rejects_launcher_cluster_perl_node_and_data_url_forms() {
        let root = tempfile::tempdir().unwrap();
        let _guard = install(false, &[AllowTarget::BashVerify]);
        for command in [
            "bash -oc pipefail 'cp a b'",
            "bash -Oc extglob 'cp a b'",
            "bash -eoc pipefail 'cp a b'",
            "env FOO=1 bash -oc pipefail 'cp a b'",
            "perl -M File::Copy -e 'print 1'",
            "perl -MFile::Copy -m Foo -e 1",
            "flock lockfile sh -c 'cp a b'",
            "chrt 1 sh -c 'cp a b'",
            "taskset 1 sh -c 'cp a b'",
            "unshare sh -c 'cp a b'",
            "strace -o /dev/null sh -c 'cp a b'",
            "watch -n1 sh -c 'cp a b'",
            "npx sh -c 'cp a b'",
            "npm exec -- sh -c 'cp a b'",
            "pnpm exec sh -c 'cp a b'",
            "npx -c 'cp a b'",
            "npx --call 'cp a b'",
            "npm exec -c 'cp a b'",
            "flock lockfile -c 'cp a b'",
            "script -qc 'cp a b' /dev/null",
            "node --import data:text/javascript,1 smoke-check.js",
            "node --import=data:text/javascript,1 smoke-check.js",
            "node -r data:text/javascript,1 smoke-check.js",
            "node --loader=data:text/javascript,1 smoke-check.js",
            "node --experimental-loader data:text/javascript,1 smoke-check.js",
            "perl -M'5;system(1)' script.pl",
            "perl -m'Foo bar' script.pl",
            "node --unknown -e 1",
            "node --foo=1 -p 1",
            "node --inspect-port 1 -e 1",
        ] {
            let arguments = json!({ "command": command });
            assert!(
                ensure_current_allows("Bash", &arguments, root.path()).is_err(),
                "{command}"
            );
        }
        for command in [
            "bash -o pipefail smoke-check.sh",
            "bash -O extglob smoke-check.sh",
            "perl -M File::Copy script.pl -e",
            "perl -MFile::Copy script.pl",
            "perl -MFoo=a,b script.pl",
            "perl -M-strict script.pl",
            "node --trace-uncaught smoke-check.js -e",
            "node --trace-uncaught smoke-check.js",
            "node --no-warnings --trace-exit smoke-check.js -p",
            "node --require ./setup.js smoke-check.js",
            "node --import ./setup.mjs smoke-check.mjs",
            "npx tsc --noEmit",
            "npx tsc",
            "npx vitest run",
            "npx vitest run -c vitest.config.ts",
            "npm exec -- tsc --noEmit",
            "npm test",
            "pnpm exec tsc --noEmit",
            "echo sh -c",
            "grep -r 'sh -c' src",
            "grep 'node -e' README.md",
        ] {
            let arguments = json!({ "command": command });
            assert!(
                ensure_current_allows("Bash", &arguments, root.path()).is_ok(),
                "{command}"
            );
        }
    }

    #[test]
    fn issue871_bash_verify_handles_nested_launcher_string_options() {
        let root = tempfile::tempdir().unwrap();
        let _guard = install(false, &[AllowTarget::BashVerify]);
        for command in [
            "chrt 1 npx -c 'cp a b'",
            "taskset 1 npm exec -c 'cp a b'",
            "setsid script -qc 'cp a b' /dev/null",
            "nice -n 5 chrt 1 flock f -c 'cp a b'",
            "timeout 5 strace -o /dev/null npx -c 'cp a b'",
            "chrt 1 taskset 1 setsid sh -c 'cp a b'",
        ] {
            let arguments = json!({ "command": command });
            assert!(
                ensure_current_allows("Bash", &arguments, root.path()).is_err(),
                "{command}"
            );
        }
        for command in [
            "chrt 1 npx tsc --noEmit",
            "taskset -c 0 npm exec -- tsc --noEmit",
            "nice -n 5 chrt 1 npx vitest run",
            "timeout 5 strace -o /dev/null npm test",
        ] {
            let arguments = json!({ "command": command });
            assert!(
                ensure_current_allows("Bash", &arguments, root.path()).is_ok(),
                "{command}"
            );
        }
    }

    #[test]
    fn issue871_bash_verify_handles_unresolved_prefix_option_value_and_stdin_shell_forms() {
        let root = tempfile::tempdir().unwrap();
        let _guard = install(false, &[AllowTarget::BashVerify]);
        for command in [
            "nice -n 5 npx -c 'cp a b'",
            "nice -n 5 flock f -c 'cp a b'",
            "nice -n 5 npm exec -c 'cp a b'",
            "nice -n 5 sh -c 'cp a b'",
            "nice -n5 npx -c 'cp a b'",
            "nice -n5 flock f -c 'cp a b'",
            "nice -n5 npm exec -c 'cp a b'",
            "nice -n5 sh -c 'cp a b'",
            "timeout -s KILL 5 npx -c 'cp a b'",
            "timeout -s KILL 5 flock f -c 'cp a b'",
            "timeout -s KILL 5 npm exec -c 'cp a b'",
            "timeout -s KILL 5 sh -c 'cp a b'",
            "timeout -sKILL 5 npx -c 'cp a b'",
            "timeout -sKILL 5 flock f -c 'cp a b'",
            "timeout -sKILL 5 npm exec -c 'cp a b'",
            "timeout -sKILL 5 sh -c 'cp a b'",
            "npm exec -p -c 'cp a b'",
            "npm x -p --call 'cp a b'",
            "npx -p -c 'cp a b'",
            "chrt -o 0 bash -sc 'cp a b'",
            "chrt -o 0 bash -s -c 'cp a b'",
            "taskset 1 sh -cs 'cp a b'",
            "bash -sc 'cp a b'",
            "bash -s -c 'cp a b'",
        ] {
            let arguments = json!({ "command": command });
            assert!(
                ensure_current_allows("Bash", &arguments, root.path()).is_err(),
                "{command}"
            );
        }
        for command in [
            "npx -p typescript tsc --noEmit",
            "npm exec -p -- tsc --noEmit",
            "nice -n5 npx tsc --noEmit",
            "timeout -sKILL 5 npx vitest run",
            "chrt 1 npx tsc --noEmit",
        ] {
            let arguments = json!({ "command": command });
            assert!(
                ensure_current_allows("Bash", &arguments, root.path()).is_ok(),
                "{command}"
            );
        }
    }

    #[test]
    fn issue871_bash_verify_keeps_canonical_verifiers() {
        let root = tempfile::tempdir().unwrap();
        let _guard = install(false, &[AllowTarget::BashVerify]);
        for command in [
            "npm test",
            "npm run build",
            "npx tsc",
            "npx tsc --noEmit",
            "node smoke-check.js",
            "node --check smoke-check.js",
            "node smoke-check.js --label=-e",
            "pytest",
            "python3 -m pytest",
            "python3 -m compileall -q src",
            "sh smoke-check.sh",
            "sed -n '1,5p' src/a.ts",
            "sed -n 1,10p src/a.ts",
            "find . -name '*.ts'",
            "grep -n 'node -e' src/a.ts",
        ] {
            let arguments = json!({ "command": command });
            assert!(
                ensure_current_allows("Bash", &arguments, root.path()).is_ok(),
                "{command}"
            );
        }
    }
}
