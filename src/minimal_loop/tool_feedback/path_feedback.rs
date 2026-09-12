pub(super) fn missing_read(name: &str, error: &anyhow::Error) -> Option<String> {
    crate::tools::read_missing::from_error(error)?;
    Some(format!(
        "Tool call `{name}` failed: {error}. Treat this path as absent and continue diagnosis using existing files. Do not repeat an unchanged missing Read. Create the file with Write only if the original task requires it and the current step permits writing; a read-only inspection must only report the absence. The original required paths and registered verification checks remain mandatory."
    ))
}

pub(super) fn confinement_retry_guidance(err_text: &str) -> Option<String> {
    let marker = "use workspace-relative path `";
    let nearest = err_text.split_once(marker)?.1.split('`').next()?;
    (!nearest.is_empty()).then(|| crate::tools::bash::workspace_relative_retry_guidance(nearest))
}
