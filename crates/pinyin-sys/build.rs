use std::env;
use std::path::PathBuf;

fn main() {
    let lib = pkg_config::Config::new()
        .atleast_version("2.6")
        .probe("libpinyin")
        .unwrap_or_else(|e| {
            panic!(
                "libpinyin development files not found ({e}).\n\
                 On Pop!_OS / Debian / Ubuntu: sudo apt install libpinyin15-dev"
            )
        });

    let mut builder = bindgen::Builder::default()
        .header_contents("wrapper.h", "#include <stdbool.h>\n#include <pinyin.h>")
        .allowlist_function("pinyin_.*")
        .allowlist_type("(pinyin|lookup|sort|_pinyin|Pinyin|Double|Full|Zhuyin|export_).*")
        .allowlist_var("(IS_PINYIN|IS_ZHUYIN|PINYIN_.*|ZHUYIN_.*|USE_.*|FORCE_TONE|DYNAMIC_ADJUST|DOUBLE_PINYIN_.*|FULL_PINYIN_.*|SORT_.*|NBEST_MATCH_CANDIDATE|NORMAL_CANDIDATE|ZOMBIE_CANDIDATE|PREDICTED_.*|ADDON_CANDIDATE|LONGER_CANDIDATE)")
        .prepend_enum_name(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));
    for path in &lib.include_paths {
        builder = builder.clang_arg(format!("-I{}", path.display()));
    }
    // Debian keeps the data under /usr/lib/<triplet>/libpinyin, not /usr/share.
    if let Ok(dir) = pkg_config::get_variable("libpinyin", "pkgdatadir") {
        println!("cargo:rustc-env=LIBPINYIN_PKGDATADIR={dir}");
    }
    let bindings = builder
        .generate()
        .expect("failed to generate libpinyin bindings");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");
    bindings
        .write_to_file(out)
        .expect("failed to write bindings.rs");
}
