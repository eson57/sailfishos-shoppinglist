use cxx_qt_lib::QString;
use gettextrs::{gettext, ngettext, pgettext};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        type Translator = super::TranslatorRust;

        #[qinvokable]
        fn tr(self: &Translator, text: &QString) -> QString;

        #[qinvokable]
        fn ctr(self: &Translator, context: &QString, text: &QString) -> QString;

        #[qinvokable]
        fn trn(self: &Translator, singular: &QString, plural: &QString, n: i32) -> QString;
    }

    #[namespace = "rust::cxxqtlib1"]
    unsafe extern "C++" {
        include!("cxx-qt-lib/common.h");

        #[cxx_name = "make_unique"]
        fn new_translator() -> UniquePtr<Translator>;
    }
}

pub use qobject::new_translator;

#[derive(Default)]
pub struct TranslatorRust;

impl qobject::Translator {
    fn tr(&self, text: &QString) -> QString {
        QString::from(gettext(text.to_string()))
    }

    fn ctr(&self, context: &QString, text: &QString) -> QString {
        QString::from(pgettext(context.to_string(), text.to_string()))
    }

    fn trn(&self, singular: &QString, plural: &QString, n: i32) -> QString {
        QString::from(ngettext(
            singular.to_string(),
            plural.to_string(),
            n.max(0) as u32,
        ))
    }
}
