//! In-process generation provenance, scoped to one UltraPlan execution.
//! A filename, goal, or old artifact is never proof of current-run authority.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use crate::config::Config;

#[derive(Default)]
struct GeneratedAuthority {
    expected_run_path: PathBuf,
    run_path: Option<PathBuf>,
    step_path: Option<PathBuf>,
}

thread_local! {
    static GENERATED_AUTHORITY: RefCell<Option<GeneratedAuthority>> = const { RefCell::new(None) };
}

pub(crate) struct RunAuthorityGuard(Option<GeneratedAuthority>);

impl Drop for RunAuthorityGuard {
    fn drop(&mut self) {
        GENERATED_AUTHORITY.with(|state| *state.borrow_mut() = self.0.take());
    }
}

pub(crate) fn begin_run(config: &Config) -> RunAuthorityGuard {
    let authority = GeneratedAuthority {
        expected_run_path: super::generated_path(config, super::ULTRA_RUN_CONTRACT),
        ..GeneratedAuthority::default()
    };
    RunAuthorityGuard(GENERATED_AUTHORITY.with(|state| state.replace(Some(authority))))
}

pub(crate) fn enter_run(config: &Config) -> Option<RunAuthorityGuard> {
    (!has_run_scope(config)).then(|| begin_run(config))
}

pub(super) fn has_run_scope(config: &Config) -> bool {
    GENERATED_AUTHORITY.with(|state| {
        state.borrow().as_ref().is_some_and(|authority| {
            authority.expected_run_path == super::generated_path(config, super::ULTRA_RUN_CONTRACT)
        })
    })
}

/// Called only after the product generated and saved a contract. Configured
/// contracts return from the acceptance binder before reaching this boundary.
pub(crate) fn record_generated_contract(config: &Config, scope: &str, path: &Path) {
    GENERATED_AUTHORITY.with(|state| {
        let mut state = state.borrow_mut();
        let Some(authority) = state.as_mut() else {
            return;
        };
        if authority.expected_run_path != super::generated_path(config, super::ULTRA_RUN_CONTRACT) {
            return;
        }
        match scope {
            "ultra-plan-run" => authority.run_path = path.canonicalize().ok(),
            "plan-run" => authority.step_path = path.canonicalize().ok(),
            _ => {}
        }
    });
}

pub(super) fn owns_run_contract(path: &Path) -> bool {
    owns_contract(path, false)
}

pub(super) fn owns_step_contract(path: &Path) -> bool {
    owns_contract(path, true)
}

fn owns_contract(path: &Path, step: bool) -> bool {
    GENERATED_AUTHORITY.with(|state| {
        let state = state.borrow();
        let Some(authority) = state.as_ref() else {
            return false;
        };
        let owned = if step {
            &authority.step_path
        } else {
            &authority.run_path
        };
        owned
            .as_ref()
            .is_some_and(|owned| path.canonicalize().is_ok_and(|path| &path == owned))
    })
}
