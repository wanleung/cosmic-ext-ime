#![allow(
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    clippy::all
)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

extern "C" {
    /// glib's allocator, used to free strings libpinyin hands out.
    pub fn g_free(mem: *mut std::os::raw::c_void);
}

/// libpinyin's installed data directory (pkg-config `pkgdatadir`), without the
/// trailing `/data` component.
pub const PKGDATADIR: &str = match option_env!("LIBPINYIN_PKGDATADIR") {
    Some(d) => d,
    None => "/usr/share/libpinyin",
};
