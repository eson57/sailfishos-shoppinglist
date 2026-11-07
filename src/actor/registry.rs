use actor_macro::with_executor;
use cxx_qt::CxxQtType;
use ractor::ActorRef;
use std::cell::{Cell, RefCell};
use std::pin::Pin;
use std::sync::Arc;

use shoppinglist_core::mesh::{local_state::LocalStateMsg, synced_state::SyncedStateMsg};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "RustQt" {
        #[qobject]
        type ActorRegistry = super::ActorRegistryRust;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qinvokable]
        fn finish_shutdown(self: &ActorRegistry);
    }

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn new_actor_registry() -> UniquePtr<ActorRegistry>;
    }

    #[namespace = "shoppinglist"]
    unsafe extern "C++" {
        include!("sailfishos-shoppinglist/src/cpp/app.h");

        #[cxx_name = "connectAboutToQuit"]
        fn connect_about_to_quit(registry: Pin<&mut ActorRegistry>);
    }
}

pub use qobject::ActorRegistry;

#[derive(Clone)]
pub struct Actors {
    pub local_state: ActorRef<LocalStateMsg>,
    pub synced_state: ActorRef<SyncedStateMsg>,
}

impl Actors {
    pub fn new(
        local_state: ActorRef<LocalStateMsg>,
        synced_state: ActorRef<SyncedStateMsg>,
    ) -> Self {
        Self {
            local_state,
            synced_state,
        }
    }
}

#[derive(Default)]
pub struct ActorRegistryRust {
    actors: Option<Arc<Actors>>,
    shutting_down: Cell<bool>,
    shutdown_enqueued: Cell<bool>,

    #[allow(clippy::type_complexity)]
    deferred_connections: RefCell<Vec<Box<dyn FnOnce(Arc<Actors>)>>>,
}

impl ActorRegistry {
    pub fn from_actors(actors: Arc<Actors>) -> cxx::UniquePtr<Self> {
        let mut registry = qobject::new_actor_registry();
        registry.pin_mut().rust_mut().actors = Some(actors);
        registry
    }

    pub fn finish_shutdown_on_quit(self: Pin<&mut Self>) {
        qobject::connect_about_to_quit(self);
    }

    #[with_executor]
    pub fn finish_shutdown(&self) {
        self.begin_shutdown();

        if self.shutdown_enqueued.replace(true) {
            return;
        }

        let Some(actors) = self.actors.as_ref().cloned() else {
            return;
        };

        tokio::task::spawn_local(async move {
            if let Err(err) = actors
                .synced_state
                .get_cell()
                .stop_and_wait(None, None)
                .await
            {
                tracing::warn!("Synced state did not stop cleanly: {err:?}");
            }
            if let Err(err) = actors
                .local_state
                .get_cell()
                .stop_and_wait(None, None)
                .await
            {
                tracing::warn!("Local state did not stop cleanly: {err:?}");
            }
        });
    }
}

#[allow(dead_code)]
impl ActorRegistryRust {
    pub fn connect(&self) {
        for cb in self.deferred_connections.take() {
            let actors_arc = self.actors.as_ref().unwrap().clone();
            tokio::task::spawn_local(async move { cb(actors_arc) });
        }
    }

    pub fn local_state_actor(&self) -> ActorRef<LocalStateMsg> {
        self.actors.as_ref().map(|a| a.local_state.clone()).unwrap()
    }

    pub fn synced_state_actor(&self) -> ActorRef<SyncedStateMsg> {
        self.actors
            .as_ref()
            .map(|a| a.synced_state.clone())
            .unwrap()
    }

    pub fn deferred_connect(&self, cb: impl FnOnce(Arc<Actors>) + 'static) {
        match self.actors.as_ref() {
            Some(actors) => {
                let actors_arc = actors.clone();
                tokio::task::spawn_local(async move { cb(actors_arc) });
            }
            _ => self.deferred_connections.borrow_mut().push(Box::new(cb)),
        }
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.get()
    }

    pub fn begin_shutdown(&self) {
        self.shutting_down.set(true);
    }
}
