use std::path::Path;

use commandagent::tui::boundary_shell::confirmation::ConfirmationIdentity;

pub(super) const EXECUTION_ROOT_LABEL: &str = "<execution-root>";

pub(super) fn identity(identity: &ConfirmationIdentity) -> ConfirmationIdentity {
    let mut projected = identity.clone();
    projected.workspace = EXECUTION_ROOT_LABEL.to_string();
    projected
}

pub(super) fn text(value: impl Into<String>, execution_root: &Path) -> String {
    let execution_root = execution_root.to_string_lossy();
    let mut projected = value
        .into()
        .replace(execution_root.as_ref(), EXECUTION_ROOT_LABEL);
    if let Some(alias) = execution_root.strip_prefix("/private/") {
        projected = projected.replace(&format!("/{alias}"), EXECUTION_ROOT_LABEL);
    }
    projected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_every_execution_root_occurrence() {
        let root = Path::new("/private/tmp/trial-root");

        assert_eq!(
            text(
                "workspace=/private/tmp/trial-root; events=/private/tmp/trial-root/.commandagent/runs/one/events.jsonl",
                root,
            ),
            "workspace=<execution-root>; events=<execution-root>/.commandagent/runs/one/events.jsonl"
        );
    }

    #[test]
    fn redacts_the_macos_private_path_alias() {
        assert_eq!(
            text(
                "workspace=/var/folders/example/trial-root",
                Path::new("/private/var/folders/example/trial-root"),
            ),
            "workspace=<execution-root>"
        );
    }
}
