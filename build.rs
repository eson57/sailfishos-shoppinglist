use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::{env, path::Path};

use cxx_qt_build::CxxQtBuilder;
use itertools::Itertools as _;
use walkdir::WalkDir;

fn update_language_files(domain: &str) {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let pot_file = manifest_dir.join(format!("po/{domain}.pot"));
    let source_files = source_files(&manifest_dir);

    // Run from the manifest directory with relative paths, so that the source
    // references in the catalogs don't contain the build machine's paths.
    let status = Command::new("xgettext")
        .current_dir(&manifest_dir)
        .args([
            &format!("--output=po/{domain}.pot"),
            "--language=javascript",
            "--qt",
            "--keyword=tr",
            "--keyword=tr:1,2",
            "--keyword=ctr:1c,2",
            "--keyword=trn:1,2",
            "--keyword=QT_TR_NOOP",
            "--keyword=npgettext",
            "--keyword=npgettext:1,2",
            "--add-comments=i18n",
            "--from-code=UTF-8",
        ])
        .args(&source_files)
        .status()
        .expect("xgettext");
    assert!(status.success(), "xgettext failed");
    strip_creation_date(&pot_file);

    let install_dir = env::var("INSTALL_DIR")
        .ok()
        .filter(|dir| !dir.is_empty())
        .unwrap_or_else(|| manifest_dir.join("target/install").display().to_string());

    for po_file in po_files(&manifest_dir) {
        let po_file_name = po_file
            .to_str()
            .expect("po language file name contains invalid characters");

        let status = Command::new("msgmerge")
            .args(["--update", po_file_name, pot_file.to_str().unwrap()])
            .status()
            .expect("msgmerge");
        assert!(status.success(), "msgmerge failed");
        strip_creation_date(&po_file);

        let lang = po_file.file_stem().unwrap().to_str().unwrap();
        let mo_dir = format!("{install_dir}/share/locale/{lang}/LC_MESSAGES");
        let mo_file = format!("{mo_dir}/{domain}.mo");
        fs::create_dir_all(&mo_dir).expect("locale directory");

        let status = Command::new("msgfmt")
            .args([po_file_name, "-o", &mo_file])
            .status()
            .expect("msgfmt");
        assert!(status.success(), "msgfmt failed");
    }
}

fn strip_creation_date(path: &Path) {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("Failed to read {}: {err}", path.display()));
    let stripped: String = contents
        .split_inclusive('\n')
        .filter(|line| !line.starts_with("\"POT-Creation-Date:"))
        .collect();
    if stripped != contents {
        fs::write(path, stripped)
            .unwrap_or_else(|err| panic!("Failed to write {}: {err}", path.display()));
    }
}

fn source_files(root: &PathBuf) -> Vec<PathBuf> {
    let mut files = walk_dir(root.join("qml"), "qml");
    files.append(&mut walk_dir(root.join("src"), "rs"));
    let mut files: Vec<_> = files
        .into_iter()
        .map(|file| file.strip_prefix(root).unwrap_or(&file).to_path_buf())
        .collect();
    files.sort();
    files
}

fn po_files(root: &Path) -> Vec<PathBuf> {
    walk_dir(root.join("po"), "po")
}

fn walk_dir(dir: PathBuf, ext: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries = fs::read_dir(&dir).unwrap_or_else(|err| {
        panic!("Failed to iterate over {}: {err}", dir.display());
    });

    for entry in entries.filter_map(Result::ok) {
        if entry.file_type().unwrap().is_dir() {
            files.append(&mut walk_dir(entry.path(), ext));
        } else if let Some(file_ext) = entry.path().extension()
            && file_ext == ext
        {
            files.push(entry.path());
        }
    }

    files
}

fn rs_files(root: &str) -> impl Iterator<Item = PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        .filter(|e| {
            fs::read_to_string(e.path())
                .is_ok_and(|src| src.lines().any(|l| l.starts_with("#[cxx_qt::bridge")))
        })
        .map(|e| e.into_path())
}

fn export_identity() -> String {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_dir.join("Cargo.toml"))
        .no_deps()
        .exec()
        .expect("cargo metadata");
    let sailjail = &metadata.workspace_metadata["sailjail"];
    let field = |name: &str| {
        sailjail[name]
            .as_str()
            .unwrap_or_else(|| panic!("workspace.metadata.sailjail.{name} missing in Cargo.toml"))
            .to_owned()
    };
    let application = field("application");

    let bin = metadata
        .root_package()
        .and_then(|package| package.targets.iter().find(|t| t.is_bin()))
        .expect("root package has a binary target");
    assert_eq!(
        bin.name, application,
        "sailjail application name must match the binary name"
    );

    println!(
        "cargo:rustc-env=SAILJAIL_ORGANIZATION={}",
        field("organization")
    );
    println!("cargo:rustc-env=SAILJAIL_APPLICATION={application}");
    println!("cargo:rerun-if-changed=Cargo.toml");
    application
}

#[derive(serde::Serialize)]
struct CrateAttribution {
    name: String,
    version: String,
    authors: Vec<String>,
    /// Indices into the list of SPDX licenses.
    licenses: Vec<usize>,
    repository: Option<String>,
    homepage: Option<String>,
}

fn generate_crate_attributions() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let config = cargo_about::licenses::config::Config {
        targets: vec![env::var("TARGET").expect("TARGET")],
        ignore_build_dependencies: true,
        ignore_dev_dependencies: true,
        ..Default::default()
    };

    let manifest_path = krates::Utf8PathBuf::try_from(manifest_dir.join("Cargo.toml")).unwrap();
    let lock_options = krates::LockOptions {
        frozen: false,
        locked: false,
        offline: false,
    };
    let krates = cargo_about::get_all_crates(
        &manifest_path,
        false,
        false,
        Vec::new(),
        false,
        lock_options,
        &config,
        &[],
    )
    .expect("gathering crates");

    let mut spdx_ids: Vec<&str> = Vec::new();
    let mut spdx_indices: HashMap<&str, usize> = HashMap::new();
    let mut crates: Vec<_> = krates
        .krates()
        .filter(|krate| {
            !krate
                .publish
                .as_ref()
                .is_some_and(|publish| publish.is_empty())
        })
        .map(|krate| {
            let license = krate
                .license
                .as_ref()
                .unwrap_or_else(|| panic!("{krate}: no license field"));
            let expression = cargo_about::parse_license_expression(license)
                .unwrap_or_else(|err| panic!("{krate}: {err}"));
            (krate, expression)
        })
        .map(|(package, expression)| CrateAttribution {
            licenses: expression
                .requirements()
                .filter_map(|requirement| requirement.req.license.id())
                .map(|id| {
                    *spdx_indices.entry(id.name).or_insert_with(|| {
                        spdx_ids.push(id.name);
                        spdx_ids.len() - 1
                    })
                })
                .unique()
                .collect(),
            name: package.name.clone(),
            version: package.version.to_string(),
            authors: package.authors.clone(),
            repository: package.repository.clone(),
            homepage: package
                .homepage
                .clone()
                .filter(|homepage| package.repository.as_ref() != Some(homepage)),
        })
        .collect();
    crates.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));

    let mut env = minijinja::Environment::new();
    env.add_template(
        "CrateAttributions.qml",
        include_str!("build/CrateAttributions.qml.j2"),
    )
    .expect("CrateAttributions template");
    let qml = env
        .get_template("CrateAttributions.qml")
        .unwrap()
        .render(minijinja::context! { licenses => minijinja::value::Serde(&spdx_ids), crates => minijinja::value::Serde(&crates) })
        .expect("rendering CrateAttributions.qml");

    write_if_changed(
        &manifest_dir.join("qml/generated/CrateAttributions.qml"),
        &qml,
    );
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=build");
}

fn write_if_changed(path: &Path, contents: &str) {
    if fs::read_to_string(path).is_ok_and(|existing| existing == contents) {
        return;
    }
    fs::create_dir_all(path.parent().unwrap()).expect("creating output directory");
    fs::write(path, contents)
        .unwrap_or_else(|err| panic!("Failed to write {}: {err}", path.display()));
}

fn main() {
    // The gettext domain is the application name.
    let domain = export_identity();

    update_language_files(&domain);
    generate_crate_attributions();

    CxxQtBuilder::new()
        .qt_module("Qml")
        .file("src/actor/registry.rs")
        .files(rs_files("src/gui"))
        .cpp_file("src/cpp/app.cpp")
        .build();

    if let Ok(lib_dir) = std::env::var("QT_LIBRARY_PATH") {
        println!("cargo:rustc-link-search=native={lib_dir}");
        println!("cargo:rustc-link-arg={lib_dir}/libstdc++.so.6");
    }

    println!("cargo:rerun-if-changed=src/cpp");
    println!("cargo:rerun-if-changed=qml");
    println!("cargo:rerun-if-changed=po");
    println!("cargo:rerun-if-env-changed=QT_INCLUDE_PATH");
    println!("cargo:rerun-if-env-changed=QT_LIBRARY_PATH");
    println!("cargo:rerun-if-env-changed=INSTALL_DIR");
}
