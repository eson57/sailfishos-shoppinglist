use cxx_qt_build::CxxQtBuilder;

fn main() {
    let sailfish_include = std::env::var("SAILFISHAPP_INCLUDE")
        .unwrap_or_else(|_| "/usr/include/sailfishapp".to_string());

    println!("cargo:rerun-if-env-changed=SAILFISHAPP_INCLUDE");
    println!("cargo:rerun-if-env-changed=QT_LIBRARY_PATH");
    println!("cargo:rerun-if-changed=src/sailfishapp.h");

    let builder = CxxQtBuilder::new()
        .qt_module("Gui")
        .qt_module("Qml")
        .qt_module("Quick")
        .file("src/sailfishapp.rs")
        .cpp_file("src/sailfishapp.cpp");
    // SAFETY: only adds an include directory.
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.include(&sailfish_include);
        })
    };
    builder.build();

    if let Ok(lib_dir) = std::env::var("QT_LIBRARY_PATH") {
        println!("cargo:rustc-link-search=native={lib_dir}");
    }

    for lib in ["sailfishapp", "EGL"] {
        println!("cargo:rustc-link-lib={lib}");
    }
}
