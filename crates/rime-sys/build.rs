use std::env;
use std::path::PathBuf;

fn main() {
    let lib = pkg_config::Config::new()
        .atleast_version("1.8")
        .probe("rime")
        .unwrap_or_else(|e| {
            panic!(
                "librime development files not found ({e}).\n\
                 On Pop!_OS / Debian / Ubuntu: sudo apt install librime-dev"
            )
        });

    let mut builder = bindgen::Builder::default()
        .header_contents("wrapper.h", "#include <rime_api.h>")
        .allowlist_function("rime_get_api")
        .allowlist_type("Rime.*")
        .allowlist_var("RIME_.*")
        .prepend_enum_name(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));
    for path in &lib.include_paths {
        builder = builder.clang_arg(format!("-I{}", path.display()));
    }
    let bindings = builder.generate().expect("failed to generate librime bindings");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");
    bindings.write_to_file(out).expect("failed to write bindings.rs");
}
