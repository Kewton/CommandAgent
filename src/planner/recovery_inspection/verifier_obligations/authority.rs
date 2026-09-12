//! Compare carried producer scope against host memory, not writable JSON alone.
use super::*;
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Seal {
    attempt_id: String,
    contract_sha256: String,
    steps: Vec<PlanStep>,
    frozen: BTreeMap<String, String>,
}
thread_local! {
    static BOUND: RefCell<BTreeMap<PathBuf, Seal>> = const { RefCell::new(BTreeMap::new()) };
}
pub(crate) struct RunGuard(BTreeMap<PathBuf, Seal>);
pub(crate) fn begin_run() -> RunGuard {
    RunGuard(BOUND.with(|s| s.borrow().clone()))
}
impl Drop for RunGuard {
    fn drop(&mut self) {
        BOUND.with(|s| *s.borrow_mut() = std::mem::take(&mut self.0));
    }
}

pub(in crate::planner::recovery_inspection) fn seal(
    config: &Config,
    contract: &CompletionContract,
    steps: &[PlanStep],
    inherited: Option<&Seal>,
) -> anyhow::Result<Seal> {
    let mut frozen = inherited.map(|s| s.frozen.clone()).unwrap_or_default();
    for path in scope::scripts(&contract.verify_commands) {
        let raw = config.workspace_root.join(&path);
        if raw.exists() {
            let resolved =
                crate::tools::path_guard::resolve_existing(&config.workspace_root, &path)?;
            let hash = format!("{:x}", Sha256::digest(std::fs::read(resolved)?));
            if let Some(before) = frozen.get(&path) {
                anyhow::ensure!(
                    before == &hash,
                    "existing verifier changed before continuation: {path}"
                );
            }
            frozen.insert(path, hash);
        }
    }
    let seal = Seal {
        attempt_id: uuid::Uuid::now_v7().to_string(),
        contract_sha256: contract_hash(config)?.context("verifier contract missing")?,
        steps: steps.to_vec(),
        frozen,
    };
    let root = config.workspace_root.canonicalize()?;
    BOUND.with(|s| s.borrow_mut().insert(root, seal.clone()));
    Ok(seal)
}

pub(in crate::planner::recovery_inspection) fn validate(
    config: &Config,
    context: Option<&InspectionContext>,
) -> anyhow::Result<()> {
    let root = config.workspace_root.canonicalize()?;
    let expected = BOUND.with(|s| s.borrow().get(&root).cloned());
    let carried = context.and_then(|c| c.verifier_seal.as_ref());
    anyhow::ensure!(
        expected.as_ref() == carried,
        "Recovery verifier context differs from host attempt provenance"
    );
    if let Some(seal) = carried {
        anyhow::ensure!(
            context.is_some_and(
                |c| c.verifier_steps == seal.steps && c.contract_sha256 == seal.contract_sha256
            ),
            "Recovery verifier original scope changed"
        );
        for (path, before) in &seal.frozen {
            let resolved = crate::tools::path_guard::resolve_existing(&root, path)?;
            anyhow::ensure!(
                resolved == root.join(path),
                "existing verifier redirected: {path}"
            );
            let after = format!("{:x}", Sha256::digest(std::fs::read(resolved)?));
            anyhow::ensure!(&after == before, "existing verifier changed: {path}");
        }
    }
    Ok(())
}

pub(crate) fn feedback(config: &Config) -> Option<String> {
    load(config)
        .err()
        .map(|e| format!("Recovery verifier preservation failed: {e}"))
}
