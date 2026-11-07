use cxx_qt_build::CxxQtBuilder;

fn main() {
    CxxQtBuilder::new()
        .files(["src/lib.rs"])
        .cpp_file("src/qt_async.cpp")
        .build()
        .export();
    println!("cargo:rerun-if-changed=src");
}
