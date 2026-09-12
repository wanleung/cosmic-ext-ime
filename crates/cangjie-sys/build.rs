use std::env;
use std::path::PathBuf;

fn main() {
    let lib = pkg_config::Config::new()
        .atleast_version("1.3")
        .probe("cangjie")
        .unwrap_or_else(|e| {
            panic!(
                "libcangjie2 development files not found ({e}).\n\
                 On Pop!_OS / Debian / Ubuntu: sudo apt install libcangjie2-dev libsqlite3-dev"
            )
        });

    let mut builder = bindgen::Builder::default()
        .header_contents("wrapper.h", "#include <cangjie.h>")
        .allowlist_function("cangjie_.*")
        .allowlist_type("Cangjie.*")
        .allowlist_var("CANGJIE_.*")
        .prepend_enum_name(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    for path in &lib.include_paths {
        builder = builder.clang_arg(format!("-I{}", path.display()));
    }

    let bindings = builder.generate().expect("failed to generate libcangjie2 bindings");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");
    bindings.write_to_file(out).expect("failed to write bindings.rs");
}
