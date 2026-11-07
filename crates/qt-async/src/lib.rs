//! Runtime for concurrent running of a Qt application with a Tokio runtime.
//!
//! This crate is adapted to cxx-qt from https://gitlab.com/rubdos/qmeta-async
//!
//!
//! MIT License
//!
//! Copyright (c) 2022 Ruben De Smet
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::task::{Context, Wake, Waker};
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::task::LocalSet;

#[cxx_qt::bridge]
mod ffi {
    #[namespace = "qt_async"]
    unsafe extern "C++" {
        include!("qt-async/src/qt_async.h");

        #[cxx_name = "installPoller"]
        fn install_poller();

        #[cxx_name = "schedulePoll"]
        fn schedule_poll();
    }

    #[namespace = "qt_async"]
    extern "Rust" {
        fn poll_local_set();
    }
}

const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

static RUNTIME: OnceLock<Handle> = OnceLock::new();
static POLL_SCHEDULED: AtomicBool = AtomicBool::new(false);

std::thread_local! {
    static LOCALSET: RefCell<LocalSet> = RefCell::new(LocalSet::new());
    static REPOLL: Cell<bool> = const { Cell::new(false) };
}

struct QtWaker;

impl Wake for QtWaker {
    fn wake(self: Arc<Self>) {
        schedule();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        schedule();
    }
}

/// Asks the Qt event loop to poll the `LocalSet`. Callable from any thread.
fn schedule() {
    if !POLL_SCHEDULED.swap(true, Ordering::AcqRel) {
        ffi::schedule_poll();
    }
}

fn with_runtime<R>(f: impl FnOnce(&Handle) -> R) -> R {
    f(RUNTIME.get().expect("qt_async runtime is not running"))
}

fn poll_local_set() {
    POLL_SCHEDULED.store(false, Ordering::Release);

    with_runtime(|rt| {
        let _guard = rt.enter();
        LOCALSET.with(|ls| {
            let Ok(mut ls) = ls.try_borrow_mut() else {
                // A task spun a nested event loop. Let the outer poll know it
                // may have missed a wakeup.
                REPOLL.set(true);
                return;
            };

            let waker = Waker::from(Arc::new(QtWaker));
            let _ = Pin::new(&mut *ls).poll(&mut Context::from_waker(&waker));
        });
    });

    if REPOLL.replace(false) {
        schedule();
    }
}

/// Runs `f` with the Tokio runtime and the Qt-driven `LocalSet` entered, so it
/// can use `tokio::task::spawn_local`, `tokio::spawn` and actor references.
///
/// Wrap every entry point from Qt into Rust with this (or `#[with_executor]`).
pub fn with_executor<R>(f: impl FnOnce() -> R) -> R {
    with_runtime(|rt| {
        let _rt_guard = rt.enter();
        // While the LocalSet is being polled its context is already entered.
        let _ls_guard = LOCALSET.with(|ls| ls.try_borrow().ok().map(|ls| ls.enter()));
        f()
    })
}

/// Starts the runtime, runs `f` (typically the whole Qt application) inside
/// [`with_executor`], then shuts the runtime down.
pub fn run<R>(f: impl FnOnce() -> R) -> anyhow::Result<R> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    RUNTIME
        .set(runtime.handle().clone())
        .map_err(|_| anyhow::anyhow!("qt_async runtime already initialised"))?;

    ffi::install_poller();
    // The LocalSet only registers our waker once polled, so spawns before the
    // first poll would never wake it.
    schedule();

    // The thread that calls `block_on` drives Tokio's I/O and timers.
    let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let driver = std::thread::Builder::new()
        .name("Tokio I/O driver".to_string())
        .spawn(move || {
            runtime.block_on(async move {
                let _ = shutdown_rx.await;
            })
        })?;

    let ret = with_executor(f);

    // Let shutdown work finish, then drop the remaining tasks while the
    // driver still runs: their destructors may need the runtime.
    with_runtime(|rt| {
        let _guard = rt.enter();
        let mut local_set = LOCALSET.with(|ls| ls.replace(LocalSet::new()));
        let drained = rt.block_on(tokio::time::timeout(SHUTDOWN_GRACE, &mut local_set));
        if drained.is_err() {
            tracing::debug!("Dropping tasks still running after shutdown");
        }
        drop(local_set);
    });

    drop(shutdown);
    let _ = driver.join();

    Ok(ret)
}
