use std::path::{Path, PathBuf};

use anyhow::Context;

use super::{INVESTIGATION_CONTRACT_REF, InvestigationBindingEvidence, InvestigationRunEvidence};
use crate::evidence_envelope::{EvidenceEnvelopeSpec, EvidenceFamily};

pub fn write_investigation_evidence(
    root: &Path,
    run: &InvestigationRunEvidence,
    binding: &InvestigationBindingEvidence,
) -> anyhow::Result<(PathBuf, PathBuf)> {
    let run_path = write_investigation_run(root, "investigation-run.json", run)?;
    let binding_path = write_investigation_binding(root, binding)?;
    Ok((run_path, binding_path))
}

pub(crate) fn write_investigation_run(
    root: &Path,
    evidence_name: &str,
    run: &InvestigationRunEvidence,
) -> anyhow::Result<PathBuf> {
    let path = root.join("evidence").join(evidence_name);
    std::fs::create_dir_all(path.parent().context("run evidence parent missing")?)?;
    crate::evidence_envelope::write_json(
        &path,
        run,
        EvidenceEnvelopeSpec::new(EvidenceFamily::I, "investigation_run")
            .with_source_refs([INVESTIGATION_CONTRACT_REF]),
        false,
    )?;
    Ok(path)
}

pub(crate) fn write_investigation_binding(
    root: &Path,
    binding: &InvestigationBindingEvidence,
) -> anyhow::Result<PathBuf> {
    let path = root.join("evidence/investigation-binding.json");
    std::fs::create_dir_all(path.parent().context("binding evidence parent missing")?)?;
    crate::evidence_envelope::write_json(
        &path,
        binding,
        EvidenceEnvelopeSpec::new(EvidenceFamily::I, "investigation_binding")
            .with_source_refs(["output/diagnosis.md", "evidence/investigation-run.json"]),
        false,
    )?;
    Ok(path)
}
