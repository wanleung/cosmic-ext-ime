#![allow(
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    clippy::all
)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// libchewing's installed data directory (pkg-config `datadir`).
pub const PKGDATADIR: &str = match option_env!("CHEWING_PKGDATADIR") {
    Some(dir) => dir,
    None => "/usr/share/libchewing",
};
