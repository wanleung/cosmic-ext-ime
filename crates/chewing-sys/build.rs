use std::env;
use std::path::PathBuf;

fn main() {
    let lib = pkg_config::Config::new()
        .atleast_version("0.6")
        .probe("chewing")
        .unwrap_or_else(|e| {
            panic!(
                "libchewing development files not found ({e}).\n\
                 On Pop!_OS / Debian / Ubuntu: sudo apt install libchewing3-dev"
            )
        });

    let mut builder = bindgen::Builder::default()
        .header_contents("wrapper.h", "#include <chewing/chewing.h>")
        .allowlist_function("chewing_.*")
        .allowlist_type("(Chewing|Interval).*")
        .allowlist_var(
            "(CHINESE_MODE|SYMBOL_MODE|FULLSHAPE_MODE|HALFSHAPE_MODE|MAX_SELKEY|HSU_SELKEY_.*|KEYSTROKE_.*|AUTOLEARN_.*)",
        )
        .prepend_enum_name(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    for path in &lib.include_paths {
        builder = builder.clang_arg(format!("-I{}", path.display()));
    }

    // The installed data directory (dictionary.dat, index_tree.dat, ...).
    if let Ok(dir) = pkg_config::get_variable("chewing", "datadir") {
        println!("cargo:rustc-env=CHEWING_PKGDATADIR={dir}");
    }

    let bindings = builder
        .generate()
        .expect("failed to generate libchewing bindings");
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("bindings.rs");
    bindings
        .write_to_file(out)
        .expect("failed to write bindings.rs");
}
