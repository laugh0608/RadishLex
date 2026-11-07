//! Test-only checkpoints; no production logging, hooks or persistent authority.
use super::{identity, FsError, Identity};
use std::cell::{Cell, RefCell};
use std::fs::File;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Checkpoint {
    PrivateAclRead,
    AncestorAclRead,
    AncestorOpened,
}
struct Hook {
    point: Checkpoint,
    node: (u64, u64),
    action: Box<dyn FnOnce()>,
    fired: Rc<Cell<bool>>,
}
thread_local! {
    static HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) };
}
pub(crate) struct HookGuard {
    fired: Rc<Cell<bool>>,
}
impl HookGuard {
    pub(crate) fn fired(&self) -> bool {
        self.fired.get()
    }
}
impl Drop for HookGuard {
    fn drop(&mut self) {
        HOOK.with(|hook| hook.borrow_mut().take());
    }
}
pub(crate) fn install(
    point: Checkpoint,
    target: &File,
    action: impl FnOnce() + 'static,
) -> HookGuard {
    let id = identity(target).unwrap();
    let fired = Rc::new(Cell::new(false));
    HOOK.with(|hook| {
        assert!(
            hook.borrow().is_none(),
            "nested diagnostic hooks are forbidden"
        );
        *hook.borrow_mut() = Some(Hook {
            point,
            node: (id.device, id.inode),
            action: Box::new(action),
            fired: fired.clone(),
        });
    });
    HookGuard { fired }
}
pub(crate) fn reached(point: Checkpoint, file: &File) {
    let interested = HOOK.with(|hook| hook.borrow().as_ref().is_some_and(|h| h.point == point));
    if !interested {
        return;
    }
    let id = identity(file).unwrap();
    let selected = HOOK.with(|hook| {
        let mut hook = hook.borrow_mut();
        if hook
            .as_ref()
            .is_some_and(|h| h.node == (id.device, id.inode))
        {
            hook.take()
        } else {
            None
        }
    });
    if let Some(hook) = selected {
        hook.fired.set(true);
        (hook.action)();
    }
}

#[track_caller]
pub(crate) fn failure(
    operation: &'static str,
    error: &FsError,
    errno: Option<i32>,
    before: Option<&Identity>,
    after: Option<&Identity>,
) {
    eprintln!(
        "synthetic observer filesystem failure: operation={operation} error={error:?} \
         errno={errno:?} before={before:?} after={after:?} at={}",
        std::panic::Location::caller()
    );
}
