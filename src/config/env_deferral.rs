//! Deferred `${ENV}` expansion for preset keys that are not selected.
//!
//! An unselected `[preset.*]` still gets key and value-shape validation, but
//! its `${ENV}` references are not resolved, so an unset variable there cannot
//! block startup.

use std::path::Path;

use super::PresetConfig;
use super::resolution::parse_preset_key;

thread_local! {
    static DEFER_ENV_EXPANSION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ENV_EXPANSION_DEFERRED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn env_expansion_is_deferred() -> bool {
    DEFER_ENV_EXPANSION.get()
}

pub(super) fn note_env_expansion_deferred() {
    ENV_EXPANSION_DEFERRED.set(true);
}

/// Sets the deferral flag for its scope and restores the previous state on
/// drop, including when the scope unwinds, so nested scopes cannot leak it.
struct DeferGuard {
    previous_defer: bool,
    previous_deferred: bool,
}

impl DeferGuard {
    fn enter(defer: bool) -> Self {
        Self {
            previous_defer: DEFER_ENV_EXPANSION.replace(defer),
            previous_deferred: ENV_EXPANSION_DEFERRED.replace(false),
        }
    }

    fn deferred_a_reference(&self) -> bool {
        ENV_EXPANSION_DEFERRED.get()
    }
}

impl Drop for DeferGuard {
    fn drop(&mut self) {
        DEFER_ENV_EXPANSION.set(self.previous_defer);
        ENV_EXPANSION_DEFERRED.set(self.previous_deferred);
    }
}

/// Parses one preset key. With `defer_env`, `${ENV}` references are checked
/// for syntax but not resolved (so nothing is read or registered), and a
/// value-shape error caused by the unresolved text is not reported; every
/// other validation error is unchanged.
#[allow(
    clippy::too_many_arguments,
    reason = "forwards the parse_preset_key arguments plus the defer flag"
)]
pub(super) fn parse_preset_key_deferring_env(
    path: &Path,
    line_no: usize,
    name: &str,
    key: &str,
    value: &str,
    source: &str,
    preset: &mut PresetConfig,
    defer_env: bool,
) -> anyhow::Result<()> {
    let guard = DeferGuard::enter(defer_env);
    let result = parse_preset_key(path, line_no, name, key, value, source, preset);
    if defer_env && result.is_err() && guard.deferred_a_reference() {
        return Ok(());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{
        DEFER_ENV_EXPANSION, DeferGuard, ENV_EXPANSION_DEFERRED, env_expansion_is_deferred,
    };

    #[test]
    fn flags_are_restored_after_a_panic_inside_the_scope() {
        let outcome = std::panic::catch_unwind(|| {
            let _guard = DeferGuard::enter(true);
            super::note_env_expansion_deferred();
            assert!(env_expansion_is_deferred());
            panic!("unwind through the guard");
        });
        assert!(outcome.is_err());
        assert!(!DEFER_ENV_EXPANSION.get());
        assert!(!ENV_EXPANSION_DEFERRED.get());
    }

    #[test]
    fn nested_scopes_restore_the_outer_state_and_leave_nothing_set() {
        {
            let outer = DeferGuard::enter(true);
            super::note_env_expansion_deferred();
            {
                // A selected preset expanded inside a deferred scope.
                let inner = DeferGuard::enter(false);
                assert!(!env_expansion_is_deferred());
                assert!(!inner.deferred_a_reference());
            }
            assert!(env_expansion_is_deferred());
            assert!(outer.deferred_a_reference());
            {
                let inner = DeferGuard::enter(true);
                assert!(!inner.deferred_a_reference());
                super::note_env_expansion_deferred();
            }
            assert!(env_expansion_is_deferred());
            assert!(outer.deferred_a_reference());
        }
        assert!(!env_expansion_is_deferred());
        assert!(!ENV_EXPANSION_DEFERRED.get());
    }
}
