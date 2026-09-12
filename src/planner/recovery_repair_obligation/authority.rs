//! File permissions are not provenance. Keep the expected attempt in host memory.
use super::*;
use std::cell::RefCell;
use std::path::PathBuf;

thread_local! {
    static BOUND: RefCell<BTreeMap<PathBuf, Obligation>> = const { RefCell::new(BTreeMap::new()) };
}

pub(crate) struct RunGuard(BTreeMap<PathBuf, Obligation>);

pub(crate) fn begin_run() -> RunGuard {
    RunGuard(BOUND.with(|bound| bound.borrow().clone()))
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        BOUND.with(|bound| *bound.borrow_mut() = std::mem::take(&mut self.0));
    }
}

pub(super) fn remember(config: &Config, obligation: &Obligation) -> anyhow::Result<()> {
    let root = config.workspace_root.canonicalize()?;
    BOUND.with(|bound| bound.borrow_mut().insert(root, obligation.clone()));
    Ok(())
}

pub(super) fn expected(config: &Config) -> anyhow::Result<Option<Obligation>> {
    let root = config.workspace_root.canonicalize()?;
    Ok(BOUND.with(|bound| bound.borrow().get(&root).cloned()))
}

pub(in crate::planner) fn validate_context(
    config: &Config,
    context: Option<&Obligation>,
) -> anyhow::Result<()> {
    let expected = super::load(config)?;
    ensure!(
        expected.as_ref() == context,
        "Recovery repair context differs from host attempt provenance"
    );
    Ok(())
}
