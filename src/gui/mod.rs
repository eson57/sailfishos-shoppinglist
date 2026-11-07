mod debug_status;
mod home;
mod shopping_list;

#[cxx_qt::bridge]
mod ffi {
    #[namespace = "shoppinglist"]
    unsafe extern "C++" {
        include!("sailfishos-shoppinglist/src/cpp/app.h");

        #[cxx_name = "registerQmlTypes"]
        fn register_qml_types();
    }
}

pub fn register() {
    ffi::register_qml_types();
}
