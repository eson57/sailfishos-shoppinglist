// This module is doing some funky stuff to thread the ActorRegistry through QML
// and back to us. I'm not completely sure if this is needed, perhaps it'd be
// fine for ActorRegistry to just be a global.
//
// This module is LLM munged from https://git.agnos.is/projectmoon/indra/src/branch/master/indra/src/actor/active_model.rs

use std::cell::Cell;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use tracing::warn;

use super::{ActorRegistry, ActorWiringModel, Actors};

struct ModelSlot<T> {
    model: Cell<*mut T>,
    borrowed: Cell<bool>,
}

/// A weak pointer to an active model QObject.
///
/// The pointer becomes dangling once the model's Rust state (and so the
/// QObject) is destroyed.
pub struct ModelRef<T> {
    slot: Weak<ModelSlot<T>>,
}

impl<T> Clone for ModelRef<T> {
    fn clone(&self) -> Self {
        Self {
            slot: self.slot.clone(),
        }
    }
}

impl<T> ModelRef<T> {
    /// Runs `f` on the model if it still exists.
    ///
    /// Must only be used on the Qt main thread, outside of any other access
    /// to the same model.
    pub fn with<R>(&self, f: impl FnOnce(Pin<&mut T>) -> R) -> Option<R> {
        struct Release<'a>(&'a Cell<bool>);
        impl Drop for Release<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }

        let slot = self.slot.upgrade()?;
        let model = slot.model.get();
        if model.is_null() {
            return None;
        }
        assert!(
            !slot.borrowed.replace(true),
            "active model is already borrowed"
        );
        let _release = Release(&slot.borrowed);

        // SAFETY: the slot is owned by the model, so the model is alive. Qt
        // objects never move, and `borrowed` rules out aliasing via `with`.
        // NOTE: I've not checked this, yolo
        Some(f(unsafe { Pin::new_unchecked(&mut *model) }))
    }
}

pub struct ActiveModel<T: 'static> {
    slot: Rc<ModelSlot<T>>,
    registry: *mut ActorRegistry,
    ready: bool,
}

impl<T> Default for ActiveModel<T> {
    fn default() -> Self {
        Self {
            slot: Rc::new(ModelSlot {
                model: Cell::new(std::ptr::null_mut()),
                borrowed: Cell::new(false),
            }),
            registry: std::ptr::null_mut(),
            ready: false,
        }
    }
}

impl<T: 'static> ActiveModel<T> {
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn registry(&self) -> *mut ActorRegistry {
        self.registry
    }

    pub fn model_ref(&self) -> ModelRef<T> {
        ModelRef {
            slot: Rc::downgrade(&self.slot),
        }
    }
}

pub trait ActiveModelObject: ActorWiringModel {
    fn active_model(&self) -> &ActiveModel<Self>;
    fn active_model_mut(self: Pin<&mut Self>) -> &mut ActiveModel<Self>;
    fn emit_ready_changed(self: Pin<&mut Self>);
}

pub fn set_registry<T: ActiveModelObject>(mut model: Pin<&mut T>, registry: *mut ActorRegistry) {
    let model_ptr: *mut T = unsafe { model.as_mut().get_unchecked_mut() };

    let state = model.as_mut().active_model_mut();
    state.registry = registry;
    state.slot.model.set(model_ptr);
    let this = state.model_ref();

    let Some(registry) = (unsafe { registry.as_ref() }) else {
        warn!("Actor Registry no longer exists.");
        return;
    };

    registry.deferred_connect(move |actors| {
        if this.with(|model| reinit(model, actors)).is_none() {
            warn!("Object destroyed before being initialized");
        }
    });
}

fn reinit<T: ActiveModelObject>(mut model: Pin<&mut T>, actors: Arc<Actors>) {
    model.as_mut().connect_actors(actors);
    model.as_mut().active_model_mut().ready = true;
    model.emit_ready_changed();
}
