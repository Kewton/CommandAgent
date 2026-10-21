//! Thread-local, one-shot filesystem race seam, absent from production builds.
use std::cell::RefCell;

use super::ToolContext;

type Hook = Box<dyn FnOnce(&ToolContext, &str)>;
thread_local! {
    static AFTER_SELECTION: RefCell<Option<Hook>> = const { RefCell::new(None) };
}

pub(super) struct Reset;

impl Drop for Reset {
    fn drop(&mut self) {
        AFTER_SELECTION.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

pub(super) fn install(hook: impl FnOnce(&ToolContext, &str) + 'static) -> Reset {
    AFTER_SELECTION.with(|slot| {
        assert!(slot.borrow().is_none());
        *slot.borrow_mut() = Some(Box::new(hook));
    });
    Reset
}

pub(crate) fn after_path_selection(context: &ToolContext, selected: &str) {
    let hook = AFTER_SELECTION.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook(context, selected);
    }
}
