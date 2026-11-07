use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};

use cxx::UniquePtr;
use cxx_qt::casting::Upcast;
use cxx_qt::QObject;
use cxx_qt_lib::{QString, QUrl, QVariant};

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!(<QtCore/QObject>);
        type QObject = cxx_qt::QObject;
    }

    #[namespace = "sailors"]
    unsafe extern "C++" {
        include!("sailors/src/sailfishapp.h");

        type QmlApplicationHolder;
        type QQmlEngine;

        #[cxx_name = "newQmlApplicationHolder"]
        fn new_qml_application_holder(args: &Vec<String>) -> UniquePtr<QmlApplicationHolder>;

        #[cxx_name = "setContextProperty"]
        fn set_context_property(
            app: Pin<&mut QmlApplicationHolder>,
            name: &QString,
            value: &QVariant,
        );

        #[cxx_name = "setContextObject"]
        unsafe fn set_context_object(
            app: Pin<&mut QmlApplicationHolder>,
            name: &QString,
            obj: *mut QObject,
        );

        fn engine(app: Pin<&mut QmlApplicationHolder>) -> Pin<&mut QQmlEngine>;

        #[cxx_name = "promoteGuiAppToQmlContext"]
        fn promote_gui_app_to_qml_context(app: Pin<&mut QmlApplicationHolder>, name: &QString);

        #[cxx_name = "pathTo"]
        fn path_to(path: &QString) -> QUrl;

        #[cxx_name = "addImportPath"]
        fn add_import_path(app: Pin<&mut QmlApplicationHolder>, path: &QString);

        fn exec(app: Pin<&mut QmlApplicationHolder>) -> i32;

        #[cxx_name = "setSource"]
        fn set_source(app: Pin<&mut QmlApplicationHolder>, src: &QUrl);

        #[cxx_name = "setTitle"]
        fn set_title(app: Pin<&mut QmlApplicationHolder>, title: &QString);

        #[cxx_name = "setApplicationVersion"]
        fn set_application_version(app: Pin<&mut QmlApplicationHolder>, version: &QString);

        #[cxx_name = "setQuitOnLastWindowClosed"]
        fn set_quit_on_last_window_closed(app: Pin<&mut QmlApplicationHolder>, quit: bool);

        #[cxx_name = "installDefaultTranslator"]
        fn install_default_translator(app: Pin<&mut QmlApplicationHolder>) -> i32;

        fn show(app: Pin<&mut QmlApplicationHolder>);

        #[cxx_name = "showFullScreen"]
        fn show_full_screen(app: Pin<&mut QmlApplicationHolder>);
    }
}

pub use ffi::QQmlEngine;

/// Qt is not thread safe, and the engine can only be created once and in one thread.
/// So this is a guard that will be used to panic if the engine is created twice
static HAS_ENGINE: AtomicBool = AtomicBool::new(false);

pub struct QmlApp {
    inner: UniquePtr<ffi::QmlApplicationHolder>,
}

impl Drop for QmlApp {
    fn drop(&mut self) {
        self.inner = UniquePtr::null();
        HAS_ENGINE
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .unwrap();
    }
}

impl QmlApp {
    pub fn application(name: String) -> QmlApp {
        HAS_ENGINE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .expect("There can only be one QmlEngine in the process");

        let arguments: Vec<String> = std::iter::once(name)
            .chain(std::env::args().skip(1))
            .collect();

        QmlApp {
            inner: ffi::new_qml_application_holder(&arguments),
        }
    }

    fn holder(&mut self) -> Pin<&mut ffi::QmlApplicationHolder> {
        self.inner.pin_mut()
    }

    // TODO: these methods come directly from `qmetaobject::QmlEngine`.  Some form of attribution
    // is necessary.
    /// Sets a property for this QML context (calls QQmlEngine::rootContext()->setContextProperty)
    pub fn set_property(&mut self, name: &QString, value: &QVariant) {
        ffi::set_context_property(self.holder(), name, value);
    }

    pub fn engine(&mut self) -> Pin<&mut QQmlEngine> {
        ffi::engine(self.holder())
    }

    /// Sets a property for this QML context (calls QQmlEngine::rootContext()->setContextProperty)
    ///
    /// The object must outlive the QML engine.
    // (TODO: consider making the lifetime the one of the engine, instead of static)
    pub fn set_object_property<T: Upcast<QObject>>(&mut self, name: &QString, obj: Pin<&mut T>) {
        let obj = obj.upcast_pin();
        // SAFETY: the pointer comes from a live, pinned QObject.
        unsafe {
            let obj_ptr: *mut QObject = obj.get_unchecked_mut();
            ffi::set_context_object(self.holder(), name, obj_ptr);
        }
    }

    /// Makes the main QtGuiApplication object available in this QML context under the given name.
    pub fn promote_gui_app_to_qml_context(&mut self, name: &QString) {
        ffi::promote_gui_app_to_qml_context(self.holder(), name);
    }

    pub fn path_to(path: &QString) -> QUrl {
        ffi::path_to(path)
    }

    /// Adds a directory (or URL) where the engine searches for QML modules.
    ///
    /// Must be called before [`set_source`](Self::set_source) for imports in
    /// the root component to resolve.
    pub fn add_import_path(&mut self, path: &QString) {
        ffi::add_import_path(self.holder(), path);
    }

    pub fn exec(&mut self) -> i32 {
        ffi::exec(self.holder())
    }

    pub fn set_source(&mut self, src: &QUrl) {
        ffi::set_source(self.holder(), src);
    }

    pub fn set_title(&mut self, title: &QString) {
        ffi::set_title(self.holder(), title);
    }

    pub fn set_application_version(&mut self, version: &QString) {
        ffi::set_application_version(self.holder(), version);
    }

    pub fn set_quit_on_last_window_closed(&mut self, quit: bool) {
        ffi::set_quit_on_last_window_closed(self.holder(), quit);
    }

    pub fn install_default_translator(&mut self) -> Result<(), anyhow::Error> {
        match ffi::install_default_translator(self.holder()) {
            0 => Ok(()),
            1 => {
                log::info!("Default translator loaded.");
                Ok(())
            }
            2 => anyhow::bail!("No translators found"),
            _ => unreachable!("Impossible return code from C++"),
        }
    }

    pub fn show(&mut self) {
        ffi::show(self.holder());
    }

    pub fn show_full_screen(&mut self) {
        ffi::show_full_screen(self.holder());
    }
}
