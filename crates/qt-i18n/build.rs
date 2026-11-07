use cxx_qt_build::CxxQtBuilder;

fn main() {
    CxxQtBuilder::new().files(["src/lib.rs"]).build().export();
}
