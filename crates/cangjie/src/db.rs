//! Safe wrapper around the libcangjie2 database handle.

use std::ffi::{CStr, CString};
use std::fmt;
use std::os::raw::c_char;
use std::ptr;

use cangjie_sys as sys;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V3,
    V5,
}

impl Version {
    fn raw(self) -> sys::CangjieVersion {
        match self {
            Version::V3 => sys::CANGJIE_VERSION_3,
            Version::V5 => sys::CANGJIE_VERSION_5,
        }
    }
}

/// Character-set filters. Combine with `|`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Filter(u32);

impl Filter {
    pub const BIG5: Filter = Filter(sys::CANGJIE_FILTER_BIG5);
    pub const HKSCS: Filter = Filter(sys::CANGJIE_FILTER_HKSCS);
    pub const PUNCTUATION: Filter = Filter(sys::CANGJIE_FILTER_PUNCTUATION);
    pub const CHINESE: Filter = Filter(sys::CANGJIE_FILTER_CHINESE);
    pub const ZHUYIN: Filter = Filter(sys::CANGJIE_FILTER_ZHUYIN);
    pub const KANJI: Filter = Filter(sys::CANGJIE_FILTER_KANJI);
    pub const KATAKANA: Filter = Filter(sys::CANGJIE_FILTER_KATAKANA);
    pub const HIRAGANA: Filter = Filter(sys::CANGJIE_FILTER_HIRAGANA);
    pub const SYMBOLS: Filter = Filter(sys::CANGJIE_FILTER_SYMBOLS);

    /// Big5 + HKSCS: the usual set for Hong Kong users.
    pub const HONG_KONG: Filter = Filter(Self::BIG5.0 | Self::HKSCS.0);

    pub fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for Filter {
    type Output = Filter;
    fn bitor(self, rhs: Filter) -> Filter {
        Filter(self.0 | rhs.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CangjieError {
    DbOpen,
    DbError,
    NoMem,
    Invalid,
    Unknown(i32),
}

impl fmt::Display for CangjieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CangjieError::DbOpen => write!(f, "could not open the Cangjie database"),
            CangjieError::DbError => write!(f, "Cangjie database query failed"),
            CangjieError::NoMem => write!(f, "libcangjie ran out of memory"),
            CangjieError::Invalid => write!(f, "invalid input passed to libcangjie"),
            CangjieError::Unknown(c) => write!(f, "libcangjie returned unknown error {c}"),
        }
    }
}

impl std::error::Error for CangjieError {}

fn check(code: i32) -> Result<(), CangjieError> {
    match code as u32 {
        sys::CANGJIE_OK | sys::CANGJIE_NOCHARS => Ok(()),
        sys::CANGJIE_DBOPEN => Err(CangjieError::DbOpen),
        sys::CANGJIE_DBERROR => Err(CangjieError::DbError),
        sys::CANGJIE_NOMEM => Err(CangjieError::NoMem),
        sys::CANGJIE_INVALID => Err(CangjieError::Invalid),
        _ => Err(CangjieError::Unknown(code)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Char {
    pub chchar: String,
    pub code: String,
    pub frequency: u32,
}

pub struct CangjieDb {
    raw: *mut sys::Cangjie,
}

// libcangjie is a plain sqlite handle with no thread affinity beyond
// "don't share concurrently", which &mut enforces.
unsafe impl Send for CangjieDb {}

impl CangjieDb {
    pub fn open(version: Version, filter: Filter) -> Result<Self, CangjieError> {
        let mut raw: *mut sys::Cangjie = ptr::null_mut();
        // SAFETY: cangjie_new writes a freshly allocated handle into `raw` on success.
        check(unsafe { sys::cangjie_new(&mut raw, version.raw(), filter.bits()) })?;
        if raw.is_null() {
            return Err(CangjieError::DbOpen);
        }
        Ok(CangjieDb { raw })
    }

    /// Full Cangjie lookup. `code` may contain `*` as a wildcard.
    pub fn characters(&mut self, code: &str) -> Result<Vec<Char>, CangjieError> {
        self.query(code, sys::cangjie_get_characters)
    }

    /// Single-key shortcut table: full-width punctuation, digits, space.
    /// (Quick/速成 is *not* this; it is `characters("a*b")`.)
    pub fn characters_by_shortcode(&mut self, code: &str) -> Result<Vec<Char>, CangjieError> {
        self.query(code, sys::cangjie_get_characters_by_shortcode)
    }

    fn query(
        &mut self,
        code: &str,
        f: unsafe extern "C" fn(*mut sys::Cangjie, *mut c_char, *mut *mut sys::CangjieCharList) -> i32,
    ) -> Result<Vec<Char>, CangjieError> {
        let code = CString::new(code).map_err(|_| CangjieError::Invalid)?;
        let mut list: *mut sys::CangjieCharList = ptr::null_mut();
        // SAFETY: libcangjie only reads `code`; the list is owned by us afterwards.
        let rc = unsafe { f(self.raw, code.as_ptr() as *mut c_char, &mut list) };
        let result = check(rc).map(|_| {
            let mut out = Vec::new();
            let mut node = list;
            while !node.is_null() {
                // SAFETY: nodes and their `c` payload are valid until cangjie_char_list_free.
                unsafe {
                    let c = (*node).c;
                    if !c.is_null() {
                        out.push(Char {
                            chchar: cstr_field(&(*c).chchar),
                            code: cstr_field(&(*c).code),
                            frequency: (*c).frequency,
                        });
                    }
                    node = (*node).next;
                }
            }
            out
        });
        if !list.is_null() {
            // SAFETY: list was produced by libcangjie and not freed elsewhere.
            unsafe { sys::cangjie_char_list_free(list) };
        }
        result
    }

    /// The radical (e.g. 日) for an input key (e.g. `a`).
    pub fn radical(&mut self, key: char) -> Option<String> {
        if !key.is_ascii() {
            return None;
        }
        let mut out: *mut c_char = ptr::null_mut();
        // SAFETY: cangjie_get_radical points `out` at a static string; it is never freed.
        let rc = unsafe { sys::cangjie_get_radical(self.raw, key as c_char, &mut out) };
        if rc as u32 != sys::CANGJIE_OK || out.is_null() {
            return None;
        }
        // SAFETY: out is a NUL-terminated static string.
        Some(unsafe { CStr::from_ptr(out) }.to_string_lossy().into_owned())
    }

    pub fn is_input_key(&mut self, key: char) -> bool {
        // SAFETY: plain query on a valid handle.
        key.is_ascii()
            && unsafe { sys::cangjie_is_input_key(self.raw, key as c_char) } as u32 == sys::CANGJIE_OK
    }
}

impl Drop for CangjieDb {
    fn drop(&mut self) {
        // SAFETY: raw is non-null and owned exclusively by this struct.
        unsafe { sys::cangjie_free(self.raw) };
    }
}

fn cstr_field(field: &[c_char]) -> String {
    let bytes: Vec<u8> = field
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}
