use std::collections::BTreeSet;

use super::{GuidanceTriggerCondition, ManifestError, ManifestV1};

pub(super) fn validate(manifest: &ManifestV1) -> Result<(), ManifestError> {
    validate_artifacts(manifest)?;
    validate_guidance(manifest)
}

fn validate_artifacts(manifest: &ManifestV1) -> Result<(), ManifestError> {
    let mut declared = BTreeSet::new();
    for path in &manifest.artifacts.required {
        validate_path("artifacts.required[]", path)?;
        if !declared.insert(path.as_str()) {
            return Err(invalid("artifacts", format!("duplicate path `{path}`")));
        }
    }
    let mut group_ids = BTreeSet::new();
    for group in &manifest.artifacts.groups {
        require_non_empty("artifacts.groups[].id", &group.id)?;
        if !group_ids.insert(group.id.as_str()) {
            return Err(invalid(
                "artifacts.groups[].id",
                format!("duplicate group id `{}`", group.id),
            ));
        }
        if group.paths.len() < 2 {
            return Err(invalid(
                "artifacts.groups[].paths",
                "either-of groups must contain at least two paths",
            ));
        }
        for path in &group.paths {
            validate_path("artifacts.groups[].paths[]", path)?;
            if !declared.insert(path.as_str()) {
                return Err(invalid("artifacts", format!("duplicate path `{path}`")));
            }
        }
        if !group.paths.contains(&group.preferred) {
            return Err(invalid(
                "artifacts.groups[].preferred",
                "must name one path from the group",
            ));
        }
    }
    Ok(())
}

fn validate_guidance(manifest: &ManifestV1) -> Result<(), ManifestError> {
    if manifest.guidance.variants.is_empty() {
        return Err(invalid(
            "guidance.variants",
            "must contain at least one named variant",
        ));
    }
    for (name, variant) in &manifest.guidance.variants {
        require_non_empty("guidance.variants.<name>", name)?;
        if variant.triggers.is_empty() {
            return Err(invalid(
                "guidance.variants.<name>.triggers",
                "must contain at least one trigger",
            ));
        }
        if variant.messages.is_empty() {
            return Err(invalid(
                "guidance.variants.<name>.messages",
                "must contain at least one message",
            ));
        }
        for trigger in &variant.triggers {
            let expects_values = trigger.condition != GuidanceTriggerCondition::Always;
            if expects_values == trigger.values.is_empty() {
                return Err(invalid(
                    "guidance.variants.<name>.triggers[].values",
                    "must be empty only for always, and non-empty otherwise",
                ));
            }
            let mut values = BTreeSet::new();
            for value in &trigger.values {
                require_non_empty("guidance.variants.<name>.triggers[].values[]", value)?;
                if !values.insert(value.as_str()) {
                    return Err(invalid(
                        "guidance.variants.<name>.triggers[].values",
                        format!("duplicate value `{value}`"),
                    ));
                }
            }
        }
        for (message, text) in &variant.messages {
            require_non_empty("guidance.variants.<name>.messages.<name>", message)?;
            require_non_empty("guidance.variants.<name>.messages.<message>", text)?;
        }
    }
    Ok(())
}

fn validate_path(field: &'static str, path: &str) -> Result<(), ManifestError> {
    crate::tools::path_guard::validate_workspace_relative(path)
        .map_err(|error| invalid(field, error.to_string()))
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), ManifestError> {
    if value.trim().is_empty() {
        Err(invalid(field, "must not be empty"))
    } else {
        Ok(())
    }
}

fn invalid(field: &'static str, reason: impl Into<String>) -> ManifestError {
    ManifestError::Invalid {
        field,
        reason: reason.into(),
    }
}
